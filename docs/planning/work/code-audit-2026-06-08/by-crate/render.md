## render

crates/render is the cleanest unit in the audit — a version-correct wgpu 29.0.1 viewport renderer that meets nearly every rule the rubric checks. It is egui-free, deliberately bytemuck-free (the view uniform is hand-packed at documented std140 offsets so wgpu stays the sole dependency), holds zero unwrap/expect/panic/unsafe anywhere including tests, never localizes, keeps no per-frame tracing, and treats GPU textures purely as caches. No real violations surfaced; the only confirmed findings are two info-level deferred-decision notes worth recording, neither of which blocks.

### Strengths

- No unwrap/expect/panic/unsafe/todo/println anywhere — even the GPU device-acquisition tests use let-else early returns to skip headlessly rather than unwrap, cleaner than the test exemption would have allowed (lib.rs:519-522, 566-571).
- Deliberate bytemuck-free design is faithfully executed and documented: pack_view/put/encode_color hand-pack the 96-byte std140 uniform with the why recorded at the function, matching the crates/render/CLAUDE.md rule that render keeps wgpu as its only dependency (lib.rs:324-358).
- wgpu 29.0.1 idioms are exactly right: immediate_size:0, bind_group_layouts:&[Some(&bgl)], entry_point:Some("vs_main"), compilation_options + cache:None, multiview_mask:None, TexelCopyTextureInfo/TexelCopyBufferLayout, and write_texture with a plain &[u8] slice to sidestep the 256-byte row rule (lib.rs:129-165, 237-255).
- Nearest sampling on all three filters (mag/min/mipmap) with the why cited to bible 16.4 — the single most common pixel-art rendering bug, avoided (lib.rs:117-127).
- Heavy GPU objects (pipeline, bind group layout, sampler, uniform buffer, placeholder texture) are built once in new() and reused every frame; paint() only sets pipeline/bind group and draws (lib.rs:82-195, 263-268).
- Pollster is confined to #[cfg(test)] for headless device init, the sanctioned dev/test boundary, and ships as a dev-dependency only.
- Pure decision logic (pack_view, encode_color, put, srgb_to_linear, frame_upload_is_skippable) is factored out for off-GPU unit coverage where device tests skip, with the factoring rationale recorded in doc comments — and every free fn has a focused test (lib.rs:208-210, 378-381).
- Decision-recording rule is followed throughout: the srgb field, the placeholder texture, the BlendState::REPLACE choice, the zero-area skip, and the flags-as-f32 packing each carry a // why at the spot they shaped (lib.rs:26-40, 157-159, 326-333).

### Findings

| ID | File:Lines | Severity | Category | Issue -> Fix |
|----|-----------|----------|----------|--------------|
| U6-2 | crates/render/src/lib.rs:82-195 | info | wgpu | new()/make_texture call create_render_pipeline/create_shader_module/create_texture, which by wgpu default panic on an uncaptured validation error; the crate has no Error/Result type or error scope, violating pixhaus-wgpu's guard-the-uncaptured-error rule. Fix: when the renderer grows fallible runtime paths, add a crate-local thiserror Error/Result plus an error scope or device.on_uncaptured_error around pipeline/texture creation. Acceptable at foundation stage (startup, build-once calls on controlled inputs); recorded so the deferred decision stays visible. |
| U6-1 | crates/render/src/lib.rs:215-220 | info | docs | upload_frame documents a tightly-packed (stride == width*4) precondition and hardcodes bytes_per_row: Some(width*4), but enforces it only in prose — no debug assertion, no # Panics note, so an undersized slice triggers a wgpu validation panic. Fix: add debug_assert_eq!(rgba.len(), width as usize * height as usize * 4) or a # Panics note per pixhaus-rust-conventions' rustdoc rule. Low priority; the foundation contract is documented and dirty-rect upload is the stated 8K follow-up. |

### Checked and cleared (false positives)

- U6-3 (min_binding_size: None on the 96-byte uniform layout entry) — rejected. The cited rule that fixed-size uniforms must set min_binding_size for bind-group-creation validation does not exist in the pixhaus-wgpu skill; the skill's own canonical textured-quad example writes min_binding_size: None, and the audited code matches it. A fabricated rule on already-compliant code.
