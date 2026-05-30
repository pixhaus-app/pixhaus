//! The 4-step flow-matching text-to-image pipeline.
//!
//! candle's `flux::sampling` helpers (`get_noise`, `State::new`, `unpack`, and
//! the `denoise` loop) bake in FLUX.1's geometry: 16 latent channels, a 2x2
//! latent pack, 3-axis position ids, and a `WithForward` 7-arg forward that
//! carries a CLIP pooled `vec`. FLUX.2 klein diverges on every one of those —
//! 32 VAE channels patchified to 128, a 4-axis (t, h, w, l) id scheme, and a
//! transformer forward with no pooled vector and no guidance argument (it is
//! guidance-distilled, CFG off). So the sampler is built here on candle base
//! tensor ops rather than imported. The one piece we reuse verbatim is
//! `flux::sampling::get_schedule` — the flow-matching timestep math is geometry
//! independent, and the klein checkpoint runs the schnell-style linear schedule
//! (`get_schedule(steps, None)`, 1.0 -> 0.0, no time shift; see
//! `.tmp/lf-research/03-candle-api.md` section 4 and `01-flux2-transformer.md`
//! section 1, which records `FlowMatchEulerDiscreteScheduler`, 4 steps,
//! guidance 1.0, no CFG).
//!
//! Latent geometry, target -> tokens:
//! - The VAE downsamples 8x, so the raw latent is `(b, 32, h/8, w/8)`.
//! - A `[2, 2]` patchify folds into the channel axis: `(b, 128, h/16, w/16)`.
//! - A per-channel `BatchNorm` normalizes that 128-channel latent into the
//!   transformer's input space.
//! - Packing flattens the spatial grid to a token sequence `(b, h/16 * w/16,
//!   128)` — the transformer's `img` input.
//!
//! Sampling runs in the normalized, patchified, packed space: noise is drawn as
//! a unit normal of that exact shape, the denoise loop integrates the velocity
//! field across the schedule, then we invert the prep (unpack -> denormalize ->
//! unpatchify -> VAE decode) and encode PNG bytes.

use std::io::Cursor;

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_transformers::models::flux::sampling::get_schedule;
use image::{ImageFormat, RgbImage};

use crate::FluxError;
use crate::model::FluxTransformer;
use crate::vae::Vae;

/// The distilled step count. klein is guidance-distilled to 4 flow-matching
/// steps; nothing but an explicit advanced override changes it.
pub const DISTILLED_STEPS: usize = 4;

/// The distilled guidance value. CFG is off (the transformer has no guidance
/// embedder), so this is a no-op slot the sampler never multiplies through. Kept
/// as a named constant so the advanced-override path has a default to fall back
/// to.
pub const DISTILLED_GUIDANCE: f64 = 1.0;

/// The pixel -> latent-token reduction per side: 8x VAE conv downsample times the
/// `[2, 2]` latent patchify = 16. Output dimensions are rounded to a multiple of
/// this so the latent grid is integral.
pub const LATENT_DOWNSAMPLE: usize = 16;

/// Resolved sampling parameters for one run. Built from the request's advanced
/// flag: distilled defaults unless the override is on.
#[derive(Clone, Copy, Debug)]
pub struct SampleParams {
    /// Flow-matching step count.
    pub steps: usize,
    /// Guidance scale (a no-op for klein; carried for the advanced path).
    pub guidance: f64,
}

impl Default for SampleParams {
    fn default() -> Self {
        Self {
            steps: DISTILLED_STEPS,
            guidance: DISTILLED_GUIDANCE,
        }
    }
}

/// Round a requested pixel dimension up to a multiple of [`LATENT_DOWNSAMPLE`],
/// with a one-cell floor so a zero request still produces a 16px image. Returns
/// the rounded pixel size and the latent-token count for that side.
#[must_use]
pub fn latent_grid(pixels: u32) -> (usize, usize) {
    let px = pixels as usize;
    let cells = px.div_ceil(LATENT_DOWNSAMPLE).max(1);
    (cells * LATENT_DOWNSAMPLE, cells)
}

/// Build the 4-axis position ids for the image tokens of one `lh x lw` latent
/// grid: `(t = 0, h = row, w = col, l = 0)` in row-major order, shape
/// `(lh * lw, 4)` as f32 (the rotary tables consume f32 ids).
///
/// # Errors
///
/// Returns [`FluxError::Candle`] on a tensor build failure.
pub fn image_position_ids(lh: usize, lw: usize, device: &Device) -> Result<Tensor, FluxError> {
    let mut ids = Vec::with_capacity(lh * lw * 4);
    for row in 0..lh {
        for col in 0..lw {
            // cast bounds: latent grids are far inside u32 range for any sane
            // image size, so the row/col casts cannot truncate.
            #[allow(clippy::cast_possible_truncation)]
            ids.extend_from_slice(&[0u32, row as u32, col as u32, 0]);
        }
    }
    let ids = Tensor::from_vec(ids, (lh * lw, 4), device)?.to_dtype(DType::F32)?;
    Ok(ids)
}

/// Build the 4-axis position ids for the text tokens: `(t = 0, h = 0, w = 0,
/// l = token)` so the prompt spans the `l` axis, shape `(seq, 4)` f32. FLUX.2
/// indexes text on `l` (FLUX.1 used all-zero text ids).
///
/// # Errors
///
/// Returns [`FluxError::Candle`] on a tensor build failure.
pub fn text_position_ids(seq: usize, device: &Device) -> Result<Tensor, FluxError> {
    let mut ids = Vec::with_capacity(seq * 4);
    for tok in 0..seq {
        // cast bounds: a prompt sequence length is far inside u32 range.
        #[allow(clippy::cast_possible_truncation)]
        ids.extend_from_slice(&[0u32, 0, 0, tok as u32]);
    }
    let ids = Tensor::from_vec(ids, (seq, 4), device)?.to_dtype(DType::F32)?;
    Ok(ids)
}

/// Pack a patchified latent `(b, 128, lh, lw)` into the token sequence
/// `(b, lh * lw, 128)` the transformer consumes. Row-major spatial flatten with
/// the channel axis last.
///
/// # Errors
///
/// Returns [`FluxError::Candle`] on a shape failure.
pub fn pack_latent(latent: &Tensor) -> Result<Tensor, FluxError> {
    let (b, c, lh, lw) = latent.dims4()?;
    // (b, c, lh, lw) -> (b, lh*lw, c).
    let seq = latent.reshape((b, c, lh * lw))?.transpose(1, 2)?.contiguous()?;
    Ok(seq)
}

/// Inverse of [`pack_latent`]: scatter the token sequence `(b, lh * lw, 128)`
/// back to the patchified latent `(b, 128, lh, lw)`.
///
/// # Errors
///
/// Returns [`FluxError::Candle`] on a shape failure.
pub fn unpack_latent(seq: &Tensor, lh: usize, lw: usize) -> Result<Tensor, FluxError> {
    let (b, n, c) = seq.dims3()?;
    if n != lh * lw {
        return Err(FluxError::Device(format!("unpack expects {} tokens, got {n}", lh * lw)));
    }
    // (b, lh*lw, c) -> (b, c, lh, lw).
    let latent = seq.transpose(1, 2)?.reshape((b, c, lh, lw))?.contiguous()?;
    Ok(latent)
}

/// One flow-matching integration: integrate the transformer's velocity field
/// from `t = 1` (pure noise) to `t = 0` (clean latent) across the schedule. The
/// transformer returns a velocity, and the Euler update `img += v * dt` walks
/// the sample along the probability-flow ODE. Returns the final packed latent.
///
/// `should_continue(step, total)` runs **before** each step; returning `false`
/// stops early (cancellation). The latent at the moment of the stop is returned.
///
/// # Errors
///
/// Returns [`FluxError`] on a tensor op or a transformer forward failure.
pub fn denoise<F>(
    transformer: &FluxTransformer,
    img: &Tensor,
    img_ids: &Tensor,
    txt: &Tensor,
    txt_ids: &Tensor,
    timesteps: &[f64],
    mut should_continue: F,
) -> Result<Tensor, FluxError>
where
    F: FnMut(usize, usize) -> bool,
{
    let b = img.dim(0)?;
    let dev = img.device();
    let total = timesteps.len().saturating_sub(1);
    let mut img = img.clone();
    for (step, window) in timesteps.windows(2).enumerate() {
        // Cancellation gate before the expensive forward.
        if !should_continue(step, total) {
            break;
        }
        let (t_curr, t_prev) = (window[0], window[1]);
        // The transformer takes the current time as a (b,) tensor.
        // cast bound: schedule times are in 0.0..=1.0, exact in f32.
        #[allow(clippy::cast_possible_truncation)]
        let t_vec = Tensor::full(t_curr as f32, b, dev)?.to_dtype(img.dtype())?;
        let pred = transformer.forward(&img, img_ids, txt, txt_ids, &t_vec)?;
        // Euler step along the flow: dt is negative (time decreases), so the
        // update moves the noisy sample toward the data manifold.
        img = (img + (pred * (t_prev - t_curr))?)?;
    }
    Ok(img)
}

/// Map an image-to-image `strength` to the schedule index where denoising
/// starts. Strength `1.0` re-noises fully (start at index 0, the whole schedule —
/// equivalent to text-to-image from pure noise); strength `0.0` skips sampling
/// (start at the last index, no steps run — the init image returns unchanged).
/// In between, the start index is `round((1 - strength) * total_steps)`, so a
/// lower strength preserves more of the reference by entering the schedule later.
///
/// `total_steps` is the full step count (e.g. the distilled 4), so the schedule
/// has `total_steps + 1` points and the returned index is in `0..=total_steps`.
///
/// This is the flow-matching analogue of the diffusers `get_timesteps(strength)`
/// slice: diffusers drops `round((1 - strength) * num_steps)` leading timesteps
/// and starts sampling at the first surviving one. The init latent is noised to
/// that surviving timestep, not to `t = 1`.
#[must_use]
pub fn i2i_schedule_index(strength: f32, total_steps: usize) -> usize {
    let s = strength.clamp(0.0, 1.0);
    // skipped = round((1 - strength) * total_steps); start index = skipped.
    // cast bounds: total_steps is a small step count, the product is in
    // 0.0..=total_steps, and the round lands inside usize.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    let skipped = ((1.0 - s) * total_steps as f32).round() as usize;
    skipped.min(total_steps)
}

/// Noise a clean latent to a flow-matching time `t` along the straight-line
/// probability path: `x_t = (1 - t) * x0 + t * noise`. At `t = 1` the result is
/// pure noise; at `t = 0` it is the clean latent. This is the forward process the
/// img2img path uses to seed the partial denoise at the chosen schedule time.
///
/// # Errors
///
/// Returns [`FluxError::Candle`] on a tensor op failure.
pub fn add_noise(clean: &Tensor, noise: &Tensor, t: f64) -> Result<Tensor, FluxError> {
    // x_t = (1 - t) * x0 + t * noise.
    let signal = (clean * (1.0 - t))?;
    let noised = (noise * t)?;
    Ok((signal + noised)?)
}

/// One flow-matching integration that starts partway through the schedule and
/// optionally honors an inpaint mask. Mirrors [`denoise`] but enters at
/// `start_index` (the i2i entry point) and, when `mask`/`clean` are supplied,
/// re-pins the masked-out region to the noised reference after every step so only
/// the white-mask region is repainted.
///
/// `mask` is a packed per-token tensor `(b, n, 1)` in `0.0..=1.0`: `1.0` repaint,
/// `0.0` preserve. `clean` is the packed clean reference latent and `noise` the
/// same unit noise used to seed the entry, so the preserved region can be re-noised
/// to the exact current schedule time. When `mask` is `None` this is a plain
/// partial denoise (the i2i edit path); the inpaint path passes all three.
///
/// `should_continue(step, total)` runs before each step; `false` stops early.
///
/// # Errors
///
/// Returns [`FluxError`] on a tensor op or a transformer forward failure.
#[allow(clippy::too_many_arguments)]
pub fn denoise_partial<F>(
    transformer: &FluxTransformer,
    img: &Tensor,
    img_ids: &Tensor,
    txt: &Tensor,
    txt_ids: &Tensor,
    timesteps: &[f64],
    start_index: usize,
    mask: Option<&Tensor>,
    clean: Option<&Tensor>,
    noise: Option<&Tensor>,
    mut should_continue: F,
) -> Result<Tensor, FluxError>
where
    F: FnMut(usize, usize) -> bool,
{
    let b = img.dim(0)?;
    let dev = img.device();
    // Total reported steps is the number of windows we actually walk.
    let total = timesteps.len().saturating_sub(1).saturating_sub(start_index);
    let mut img = img.clone();
    for (offset, window) in timesteps.windows(2).skip(start_index).enumerate() {
        if !should_continue(offset, total) {
            break;
        }
        let (t_curr, t_prev) = (window[0], window[1]);
        // cast bound: schedule times are in 0.0..=1.0, exact in f32.
        #[allow(clippy::cast_possible_truncation)]
        let t_vec = Tensor::full(t_curr as f32, b, dev)?.to_dtype(img.dtype())?;
        let pred = transformer.forward(&img, img_ids, txt, txt_ids, &t_vec)?;
        img = (img + (pred * (t_prev - t_curr))?)?;
        // Inpaint: re-pin the preserved region to the reference noised to t_prev,
        // so only the white-mask area carries the new content forward.
        if let (Some(mask), Some(clean), Some(noise)) = (mask, clean, noise) {
            let reference = add_noise(clean, noise, t_prev)?;
            // out = mask * img + (1 - mask) * reference.
            let inv = (1.0 - mask)?;
            img = (img.broadcast_mul(mask)? + reference.broadcast_mul(&inv)?)?;
        }
    }
    Ok(img)
}

/// Decode a packed, denoised latent `(b, 128, ... packed)` back to RGB and
/// encode each batch element to PNG bytes.
///
/// The inverse of the latent prep, in order: unpack the token sequence to the
/// patchified latent, denormalize the `BatchNorm`, unpatchify `128 -> 32`, VAE
/// decode to `(b, 3, h, w)` in `-1..=1`, then map to `0..=255` u8 and PNG-encode
/// per image.
///
/// # Errors
///
/// Returns [`FluxError`] on a tensor op, a decode failure, or a PNG encode
/// failure.
pub fn decode_to_pngs(vae: &Vae, latent_seq: &Tensor, lh: usize, lw: usize) -> Result<Vec<Vec<u8>>, FluxError> {
    let patched = unpack_latent(latent_seq, lh, lw)?;
    // Working dtype back to f32 for the decode: the VAE is force-upcast and the
    // PNG conversion reads f32 scalars.
    let patched = patched.to_dtype(DType::F32)?;
    let denorm = vae.denormalize_latent(&patched)?;
    let raw_latent = vae.unpatchify(&denorm)?;
    let rgb = vae.decode(&raw_latent)?;
    // (b, 3, h, w) in -1..1 -> 0..255 u8.
    let rgb = rgb
        .clamp(-1.0_f64, 1.0_f64)?
        .affine(127.5, 127.5)?
        .clamp(0.0_f64, 255.0_f64)?
        .to_dtype(DType::U8)?;
    let (b, _c, h, w) = rgb.dims4()?;
    let mut out = Vec::with_capacity(b);
    for i in 0..b {
        let chw = rgb.i(i)?; // (3, h, w)
        let hwc = chw.permute((1, 2, 0))?.contiguous()?;
        let bytes = hwc.flatten_all()?.to_vec1::<u8>()?;
        // cast bound: image dimensions fit u32 by construction (rounded request).
        #[allow(clippy::cast_possible_truncation)]
        let (wu, hu) = (w as u32, h as u32);
        let image = RgbImage::from_raw(wu, hu, bytes).ok_or_else(|| FluxError::Device("RGB buffer size does not match image dimensions".to_owned()))?;
        out.push(encode_png(&image)?);
    }
    Ok(out)
}

/// Encode one `RgbImage` to PNG bytes.
fn encode_png(image: &RgbImage) -> Result<Vec<u8>, FluxError> {
    let mut buf = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| FluxError::Device(format!("PNG encode failed: {e}")))?;
    Ok(buf)
}

/// Decode PNG bytes to an NCHW f32 tensor in `-1..=1` at the working dtype, the
/// VAE's input convention. Resizes to `(out_w, out_h)` with a triangle filter so
/// the reference matches the latent grid the sampler runs on (i2i requires the
/// init latent and the noise to share a shape).
///
/// # Errors
///
/// Returns [`FluxError`] if the PNG fails to decode or a tensor op fails.
pub fn png_to_rgb_tensor(png: &[u8], out_w: u32, out_h: u32, device: &Device, dtype: DType) -> Result<Tensor, FluxError> {
    let decoded = image::load_from_memory(png).map_err(|e| FluxError::Device(format!("init image decode failed: {e}")))?;
    let resized = decoded.resize_exact(out_w, out_h, image::imageops::FilterType::Triangle).to_rgb8();
    let (w, h) = (out_w as usize, out_h as usize);
    let hwc = Tensor::from_vec(resized.into_raw(), (h, w, 3), device)?.to_dtype(DType::F32)?;
    // 0..255 -> -1..1, then HWC -> CHW -> NCHW.
    let normalized = hwc.affine(2.0 / 255.0, -1.0)?;
    let chw = normalized.permute((2, 0, 1))?;
    let nchw = chw.unsqueeze(0)?.to_dtype(dtype)?;
    Ok(nchw)
}

/// Encode an init RGB tensor (NCHW, `-1..=1`) into the normalized, patchified,
/// packed clean latent the sampler integrates in: VAE encode -> patchify ->
/// `BatchNorm` normalize -> pack to `(b, lh * lw, 128)`. The inverse of the t2i
/// decode prep, applied to a real image instead of noise.
///
/// Returns the packed latent plus its latent grid `(lh, lw)` so the caller can
/// size the noise and the unpack to match.
///
/// # Errors
///
/// Returns [`FluxError`] on a VAE or tensor op failure.
pub fn encode_init_latent(vae: &Vae, rgb: &Tensor, dtype: DType) -> Result<(Tensor, usize, usize), FluxError> {
    let raw = vae.encode(rgb)?;
    let patched = vae.patchify(&raw)?;
    let normalized = vae.normalize_latent(&patched)?;
    let (_, _, lh, lw) = normalized.dims4()?;
    let packed = pack_latent(&normalized)?.to_dtype(dtype)?;
    Ok((packed, lh, lw))
}

/// Rasterize a single-channel mask PNG into a packed per-token mask
/// `(1, lh * lw, 1)` in `0.0..=1.0`, white (255) -> `1.0` repaint, black ->
/// `0.0` preserve. The mask is resized to the latent grid (one value per latent
/// token), averaged across channels, and packed in the same token order as the
/// latent. Hard-thresholds at the latent resolution so a half-covered token
/// repaints (any white in the cell wins) rather than blending a fractional value.
///
/// # Errors
///
/// Returns [`FluxError`] if the PNG fails to decode or a tensor op fails.
pub fn mask_to_packed(png: &[u8], lh: usize, lw: usize, device: &Device, dtype: DType) -> Result<Tensor, FluxError> {
    let decoded = image::load_from_memory(png).map_err(|e| FluxError::Device(format!("mask decode failed: {e}")))?;
    // cast bounds: latent grid sizes are far inside u32.
    #[allow(clippy::cast_possible_truncation)]
    let (gw, gh) = (lw as u32, lh as u32);
    // Downsample the mask to the latent grid. Triangle averaging then a >0.5
    // threshold means any meaningful white coverage in a latent cell repaints it.
    let small = decoded.resize_exact(gw, gh, image::imageops::FilterType::Triangle).to_luma8();
    let values: Vec<f32> = small.into_raw().into_iter().map(|v| if v > 127 { 1.0 } else { 0.0 }).collect();
    // (lh*lw,) -> (1, lh*lw, 1), broadcast across the 128 channels at use.
    let mask = Tensor::from_vec(values, (1, lh * lw, 1), device)?.to_dtype(dtype)?;
    Ok(mask)
}

/// Draw the initial sampling noise: a unit normal in the normalized, patchified,
/// packed latent space, shape `(num_images, lh * lw, 128)`. Seeding is the
/// caller's job (`device.set_seed`) so a fixed seed yields a fixed image.
///
/// # Errors
///
/// Returns [`FluxError::Candle`] on a tensor build failure.
pub fn initial_noise(num_images: usize, lh: usize, lw: usize, channels: usize, device: &Device, dtype: DType) -> Result<Tensor, FluxError> {
    let noise = Tensor::randn(0f32, 1f32, (num_images, lh * lw, channels), device)?.to_dtype(dtype)?;
    Ok(noise)
}

/// Build the klein flow-matching schedule. Reuses candle's `get_schedule` math;
/// klein runs the schnell-style linear schedule (no time shift), so `shift` is
/// `None`. Returns `steps + 1` timesteps from 1.0 down to 0.0.
#[must_use]
pub fn schedule(steps: usize) -> Vec<f64> {
    get_schedule(steps, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latent_grid_rounds_up_to_downsample() {
        // Exact multiple stays put; a non-multiple rounds up; zero floors to one cell.
        assert_eq!(latent_grid(64), (64, 4));
        assert_eq!(latent_grid(1024), (1024, 64));
        assert_eq!(latent_grid(70), (80, 5));
        assert_eq!(latent_grid(0), (16, 1));
    }

    #[test]
    fn schedule_has_steps_plus_one_points_from_one_to_zero() {
        let ts = schedule(DISTILLED_STEPS);
        assert_eq!(ts.len(), DISTILLED_STEPS + 1, "4 steps -> 5 points");
        assert!((ts[0] - 1.0).abs() < 1e-12, "schedule starts at 1.0");
        assert!(ts[ts.len() - 1].abs() < 1e-12, "schedule ends at 0.0");
        // Monotone decreasing.
        assert!(ts.windows(2).all(|w| w[0] > w[1]), "schedule is strictly decreasing");
    }

    #[test]
    fn image_ids_are_cartesian_t_h_w_l() -> Result<(), FluxError> {
        // 2x3 grid: row-major (t=0, h=row, w=col, l=0).
        let device = Device::Cpu;
        let ids = image_position_ids(2, 3, &device)?;
        assert_eq!(ids.dims2()?, (6, 4));
        let rows = ids.to_dtype(DType::U32)?.to_vec2::<u32>()?;
        assert_eq!(rows[0], [0, 0, 0, 0], "first token");
        assert_eq!(rows[1], [0, 0, 1, 0], "second token: w advances");
        assert_eq!(rows[3], [0, 1, 0, 0], "fourth token: h advances, w resets");
        Ok(())
    }

    #[test]
    fn text_ids_span_the_l_axis() -> Result<(), FluxError> {
        let device = Device::Cpu;
        let ids = text_position_ids(4, &device)?;
        assert_eq!(ids.dims2()?, (4, 4));
        let rows = ids.to_dtype(DType::U32)?.to_vec2::<u32>()?;
        // t=h=w=0; l = token index.
        assert_eq!(rows[0], [0, 0, 0, 0]);
        assert_eq!(rows[3], [0, 0, 0, 3]);
        Ok(())
    }

    #[test]
    fn pack_unpack_round_trips() -> Result<(), FluxError> {
        // (1, 128, 2, 3) -> (1, 6, 128) -> (1, 128, 2, 3) with values intact.
        let device = Device::Cpu;
        let latent = Tensor::arange(0u32, 1 * 128 * 2 * 3, &device)?.to_dtype(DType::F32)?.reshape((1, 128, 2, 3))?;
        let seq = pack_latent(&latent)?;
        assert_eq!(seq.dims3()?, (1, 6, 128));
        let back = unpack_latent(&seq, 2, 3)?;
        let diff = (&latent - &back)?.abs()?.sum_all()?.to_scalar::<f32>()?;
        assert!(diff < 1e-6, "pack/unpack drifted: {diff}");
        Ok(())
    }

    #[test]
    fn initial_noise_has_packed_shape() -> Result<(), FluxError> {
        let device = Device::Cpu;
        let noise = initial_noise(2, 4, 4, 128, &device, DType::F32)?;
        assert_eq!(noise.dims3()?, (2, 16, 128), "(num_images, lh*lw, channels)");
        Ok(())
    }

    #[test]
    fn sample_params_default_is_distilled() {
        let p = SampleParams::default();
        assert_eq!(p.steps, DISTILLED_STEPS);
        assert!((p.guidance - DISTILLED_GUIDANCE).abs() < f64::EPSILON);
    }

    #[test]
    fn i2i_index_spans_strength_endpoints() {
        // Full strength enters at 0 (whole schedule == t2i); zero strength enters
        // at total_steps (no windows run); 0.6 -> round(1.6) == 2.
        assert_eq!(i2i_schedule_index(1.0, DISTILLED_STEPS), 0);
        assert_eq!(i2i_schedule_index(0.0, DISTILLED_STEPS), DISTILLED_STEPS);
        assert_eq!(i2i_schedule_index(0.6, DISTILLED_STEPS), 2);
        // Out-of-range clamps.
        assert_eq!(i2i_schedule_index(5.0, DISTILLED_STEPS), 0);
        assert_eq!(i2i_schedule_index(-1.0, DISTILLED_STEPS), DISTILLED_STEPS);
    }

    #[test]
    fn i2i_index_is_monotone_non_increasing_in_strength() {
        // A higher strength must never enter the schedule later (preserve less).
        let steps = 8;
        let mut prev = i2i_schedule_index(0.0, steps);
        for k in 1..=10 {
            let s = k as f32 / 10.0;
            let idx = i2i_schedule_index(s, steps);
            assert!(idx <= prev, "strength {s} entered later than a weaker edit");
            prev = idx;
        }
    }

    #[test]
    fn add_noise_endpoints_are_clean_and_pure_noise() -> Result<(), FluxError> {
        // t = 0 returns the clean latent; t = 1 returns the noise; t = 0.5 is the
        // straight-line midpoint.
        let device = Device::Cpu;
        let clean = Tensor::full(2.0_f32, (1, 4, 8), &device)?;
        let noise = Tensor::full(6.0_f32, (1, 4, 8), &device)?;

        let at0 = add_noise(&clean, &noise, 0.0)?;
        let d0 = (&at0 - &clean)?.abs()?.sum_all()?.to_scalar::<f32>()?;
        assert!(d0 < 1e-6, "t=0 is the clean latent: {d0}");

        let at1 = add_noise(&clean, &noise, 1.0)?;
        let d1 = (&at1 - &noise)?.abs()?.sum_all()?.to_scalar::<f32>()?;
        assert!(d1 < 1e-6, "t=1 is pure noise: {d1}");

        let mid = add_noise(&clean, &noise, 0.5)?.flatten_all()?.to_vec1::<f32>()?;
        assert!((mid[0] - 4.0).abs() < 1e-6, "midpoint is (2 + 6) / 2 = 4: {}", mid[0]);
        Ok(())
    }

    #[test]
    fn mask_packs_white_to_one_black_to_zero() -> Result<(), FluxError> {
        // A 4x4 mask: top-left 2x2 white, rest black. At a 4x4 latent grid (no
        // downsample) the packed mask is 1.0 on the white tokens, 0.0 elsewhere,
        // in row-major token order.
        let mut img = image::GrayImage::from_pixel(4, 4, image::Luma([0u8]));
        for y in 0..2 {
            for x in 0..2 {
                img.put_pixel(x, y, image::Luma([255]));
            }
        }
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .map_err(|e| FluxError::Device(format!("encode test mask: {e}")))?;

        let device = Device::Cpu;
        let mask = mask_to_packed(&png, 4, 4, &device, DType::F32)?;
        assert_eq!(mask.dims3()?, (1, 16, 1), "packed mask shape (1, lh*lw, 1)");
        let vals = mask.flatten_all()?.to_vec1::<f32>()?;
        // Tokens 0,1 (row 0 cols 0,1) and 4,5 (row 1 cols 0,1) are white.
        for &white in &[0usize, 1, 4, 5] {
            assert!((vals[white] - 1.0).abs() < 1e-6, "token {white} should repaint");
        }
        for (i, &v) in vals.iter().enumerate() {
            if ![0usize, 1, 4, 5].contains(&i) {
                assert!(v.abs() < 1e-6, "token {i} should be preserved");
            }
        }
        Ok(())
    }
}
