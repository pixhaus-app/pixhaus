//! GPU adapter preference: which wgpu adapter the editor runs on.
//!
//! eframe creates the wgpu device once at startup, before the app or its
//! Settings exist, so a chosen adapter takes effect on the next launch rather
//! than live. The choice is persisted to a small JSON file next to eframe's own
//! state (`eframe::storage_dir`), read by `main` before `run_native`, and
//! applied through a `native_adapter_selector`. Absent a preference, selection
//! is left to egui-wgpu's default.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Stable identity of a chosen adapter. Enumeration order is not stable across
/// runs and names collide (the same GPU appears under Vulkan and DX12), so an
/// adapter is pinned by `(backend, vendor, device, name)` rather than by index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuPreference {
    /// `wgpu::Backend` rendered with `{:?}` (e.g. "Vulkan", "Dx12", "Gl").
    pub backend: String,
    /// PCI-style vendor id.
    pub vendor: u32,
    /// Device id within the vendor.
    pub device: u32,
    /// Human-readable adapter name.
    pub name: String,
}

impl GpuPreference {
    /// Whether this preference identifies `info`.
    #[must_use]
    pub fn matches(&self, info: &wgpu::AdapterInfo) -> bool {
        self.vendor == info.vendor && self.device == info.device && self.name == info.name && self.backend == backend_str(info)
    }
}

/// `{:?}` of the adapter's backend, the form stored in [`GpuPreference`].
fn backend_str(info: &wgpu::AdapterInfo) -> String {
    format!("{:?}", info.backend)
}

/// Builds a [`GpuPreference`] pinning `info`.
#[must_use]
pub fn from_info(info: &wgpu::AdapterInfo) -> GpuPreference {
    GpuPreference {
        backend: backend_str(info),
        vendor: info.vendor,
        device: info.device,
        name: info.name.clone(),
    }
}

/// A one-line label for the Settings list, distinguishing the same GPU across
/// backends, e.g. `"NVIDIA GeForce RTX 5090 — Vulkan (DiscreteGpu)"`.
#[must_use]
pub fn label(info: &wgpu::AdapterInfo) -> String {
    format!("{} — {:?} ({:?})", info.name, info.backend, info.device_type)
}

/// On-disk location of the preference, a sibling of eframe's own state.
fn pref_path() -> Option<PathBuf> {
    eframe::storage_dir("pixhaus").map(|dir| dir.join("gpu.json"))
}

/// Reads the saved preference, or `None` when absent or unreadable (treated as
/// "Automatic" — let egui-wgpu pick).
#[must_use]
pub fn load() -> Option<GpuPreference> {
    let bytes = std::fs::read(pref_path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Persists `pref`, or removes the file when `None` ("Automatic"). Removing an
/// already-absent file is success.
///
/// # Errors
///
/// Propagates filesystem errors (no storage dir, write/remove failure).
pub fn save(pref: Option<&GpuPreference>) -> std::io::Result<()> {
    let path = pref_path().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no storage directory for the app"))?;
    match pref {
        Some(pref) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_vec_pretty(pref).map_err(std::io::Error::other)?;
            std::fs::write(&path, json)
        }
        None => match std::fs::remove_file(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        },
    }
}

/// Picks the adapter index for `pref` from the enumerated `infos`: the exact
/// identity match, else a sensible fallback (prefer a discrete GPU, then any
/// non-CPU adapter, then the first). `infos` must be non-empty.
#[must_use]
pub fn choose_index(pref: &GpuPreference, infos: &[wgpu::AdapterInfo]) -> usize {
    if let Some(i) = infos.iter().position(|info| pref.matches(info)) {
        return i;
    }
    if let Some(i) = infos.iter().position(|info| info.device_type == wgpu::DeviceType::DiscreteGpu) {
        return i;
    }
    if let Some(i) = infos.iter().position(|info| info.device_type != wgpu::DeviceType::Cpu) {
        return i;
    }
    0
}

/// Builds the `native_adapter_selector` closure that pins `pref` at startup,
/// falling back to a default adapter when the saved GPU is gone. Never errors
/// unless the platform reports no adapters at all.
#[must_use]
pub fn adapter_selector(pref: GpuPreference) -> egui_wgpu::NativeAdapterSelectorMethod {
    Arc::new(
        move |adapters: &[wgpu::Adapter], _surface: Option<&wgpu::Surface<'_>>| -> Result<wgpu::Adapter, String> {
            if adapters.is_empty() {
                return Err("no wgpu adapters available".to_owned());
            }
            let infos: Vec<wgpu::AdapterInfo> = adapters.iter().map(wgpu::Adapter::get_info).collect();
            Ok(adapters[choose_index(&pref, &infos)].clone())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, vendor: u32, device: u32, backend: wgpu::Backend, device_type: wgpu::DeviceType) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: name.to_owned(),
            vendor,
            device,
            device_type,
            device_pci_bus_id: String::new(),
            driver: String::new(),
            driver_info: String::new(),
            backend,
            subgroup_min_size: 0,
            subgroup_max_size: 0,
            transient_saves_memory: false,
        }
    }

    fn sample() -> Vec<wgpu::AdapterInfo> {
        vec![
            info("RTX 5090", 0x10DE, 0x2B85, wgpu::Backend::Vulkan, wgpu::DeviceType::DiscreteGpu),
            info("RTX 5090", 0x10DE, 0x2B85, wgpu::Backend::Dx12, wgpu::DeviceType::DiscreteGpu),
            info("Basic Render Driver", 0x1414, 0x8C, wgpu::Backend::Dx12, wgpu::DeviceType::Cpu),
        ]
    }

    #[test]
    fn choose_index_finds_exact_backend_match() {
        let infos = sample();
        // The same GPU under DX12 must resolve to the DX12 entry, not Vulkan.
        let pref = from_info(&infos[1]);
        assert_eq!(choose_index(&pref, &infos), 1);
    }

    #[test]
    fn choose_index_falls_back_to_discrete_when_missing() {
        let infos = sample();
        let pref = GpuPreference {
            backend: "Vulkan".to_owned(),
            vendor: 0x1002,
            device: 0x9999,
            name: "Some Removed GPU".to_owned(),
        };
        // Not present, so the first discrete GPU wins over the CPU adapter.
        assert_eq!(choose_index(&pref, &infos), 0);
    }

    #[test]
    fn choose_index_skips_cpu_when_no_discrete() {
        let infos = vec![
            info("Basic Render Driver", 0x1414, 0x8C, wgpu::Backend::Dx12, wgpu::DeviceType::Cpu),
            info("Mesa llvmpipe", 0x10005, 0x0, wgpu::Backend::Gl, wgpu::DeviceType::Other),
        ];
        let pref = GpuPreference {
            backend: "Vulkan".to_owned(),
            vendor: 0,
            device: 0,
            name: "gone".to_owned(),
        };
        // No discrete GPU, so the first non-CPU adapter wins.
        assert_eq!(choose_index(&pref, &infos), 1);
    }

    #[test]
    fn choose_index_defaults_to_first_when_all_cpu() {
        let infos = vec![info("CPU A", 1, 1, wgpu::Backend::Dx12, wgpu::DeviceType::Cpu)];
        let pref = GpuPreference {
            backend: "Vulkan".to_owned(),
            vendor: 9,
            device: 9,
            name: "gone".to_owned(),
        };
        assert_eq!(choose_index(&pref, &infos), 0);
    }

    #[test]
    fn from_info_round_trips_through_matches() {
        let infos = sample();
        let pref = from_info(&infos[0]);
        assert!(pref.matches(&infos[0]));
        // Same name/vendor/device but different backend must not match.
        assert!(!pref.matches(&infos[1]));
    }
}
