//! Device selection for the FLUX.2 runtime.
//!
//! [`DeviceChoice`] is the concrete target an inference run uses;
//! [`DevicePref`] is the persisted user setting (`Auto` lets the runtime pick).
//! Resolution walks CUDA -> Metal -> CPU, but only offers a backend the build
//! actually compiled in — the `cuda`/`metal` features gate the GPU paths, so a
//! CPU-only build always resolves to [`DeviceChoice::Cpu`].

use candle_core::Device;
use serde::{Deserialize, Serialize};

use crate::FluxError;

/// A concrete inference device. `Cuda(ordinal)` selects a specific GPU index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceChoice {
    /// An NVIDIA GPU at the given ordinal. Only reachable with the `cuda` feature.
    Cuda(usize),
    /// An Apple GPU. Only reachable with the `metal` feature.
    Metal,
    /// The CPU. Always available; minutes per image for a 4B transformer.
    Cpu,
}

/// The persisted device preference. `Auto` resolves at runtime; the explicit
/// variants force a backend (falling back to CPU if it is unavailable).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DevicePref {
    /// Pick the best available device: CUDA, then Metal, then CPU.
    #[default]
    Auto,
    /// Force CUDA at the given ordinal.
    Cuda(usize),
    /// Force Metal.
    Metal,
    /// Force CPU.
    Cpu,
}

impl DeviceChoice {
    /// Resolve the best device the build supports, walking CUDA -> Metal -> CPU.
    ///
    /// The GPU arms are compiled in only under the `cuda` / `metal` features, so
    /// a CPU-only build returns [`DeviceChoice::Cpu`] without probing hardware.
    #[must_use]
    pub fn auto() -> Self {
        #[cfg(feature = "cuda")]
        {
            return DeviceChoice::Cuda(0);
        }
        #[cfg(all(feature = "metal", not(feature = "cuda")))]
        {
            return DeviceChoice::Metal;
        }
        #[cfg(not(any(feature = "cuda", feature = "metal")))]
        {
            DeviceChoice::Cpu
        }
    }

    /// Map a persisted [`DevicePref`] to a concrete [`DeviceChoice`].
    ///
    /// `Auto` defers to [`DeviceChoice::auto`]. An explicit GPU preference that
    /// the build did not compile in falls back to CPU rather than failing.
    #[must_use]
    pub fn resolve(pref: DevicePref) -> Self {
        match pref {
            DevicePref::Auto => Self::auto(),
            DevicePref::Cuda(ordinal) => {
                #[cfg(feature = "cuda")]
                {
                    DeviceChoice::Cuda(ordinal)
                }
                #[cfg(not(feature = "cuda"))]
                {
                    let _ = ordinal;
                    DeviceChoice::Cpu
                }
            }
            DevicePref::Metal => {
                #[cfg(feature = "metal")]
                {
                    DeviceChoice::Metal
                }
                #[cfg(not(feature = "metal"))]
                {
                    DeviceChoice::Cpu
                }
            }
            DevicePref::Cpu => DeviceChoice::Cpu,
        }
    }

    /// Build the candle [`Device`] for this choice.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Device`] if the candle backend fails to initialize
    /// (for example, no CUDA runtime present despite a `Cuda` choice).
    pub fn to_device(self) -> Result<Device, FluxError> {
        let device = match self {
            DeviceChoice::Cuda(ordinal) => Device::new_cuda(ordinal).map_err(FluxError::Candle)?,
            DeviceChoice::Metal => Device::new_metal(0).map_err(FluxError::Candle)?,
            DeviceChoice::Cpu => Device::Cpu,
        };
        Ok(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a build without `cuda`/`metal`, every preference resolves to CPU and
    /// an explicit GPU preference degrades rather than failing. The CI/test gate
    /// always compiles the `cpu` feature, so this is the path under test here.
    #[cfg(not(any(feature = "cuda", feature = "metal")))]
    #[test]
    fn cpu_only_build_resolves_everything_to_cpu() {
        assert_eq!(DeviceChoice::auto(), DeviceChoice::Cpu);
        assert_eq!(DeviceChoice::resolve(DevicePref::Auto), DeviceChoice::Cpu);
        assert_eq!(DeviceChoice::resolve(DevicePref::Cuda(0)), DeviceChoice::Cpu);
        assert_eq!(DeviceChoice::resolve(DevicePref::Metal), DeviceChoice::Cpu);
        assert_eq!(DeviceChoice::resolve(DevicePref::Cpu), DeviceChoice::Cpu);
    }

    #[test]
    fn cpu_choice_builds_a_device() {
        let device = DeviceChoice::Cpu.to_device().expect("cpu device is infallible");
        assert!(matches!(device, Device::Cpu));
    }

    #[test]
    fn device_pref_default_is_auto() {
        assert_eq!(DevicePref::default(), DevicePref::Auto);
    }
}
