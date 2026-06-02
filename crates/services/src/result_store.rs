//! The result store: completed generated assets, held until applied or discarded.
//!
//! Results enter here as transient job output (bible 17.2) and stay until the user
//! applies one (through a command) or discards it. This is the one genuinely
//! shared-across-threads piece — the job task writes, the UI lane reads — so it is
//! the sanctioned `Arc<Mutex<…>>` exception to the single-owner rule. The lock is
//! never held across an `.await`: every critical section is a quick read or write.

use parking_lot::Mutex;

use crate::generated::{GeneratedAsset, GenerationProvenance};
use crate::job::JobId;

#[derive(Default)]
struct ResultState {
    /// Completed jobs in arrival order; the index into this is the tray position.
    order: Vec<JobId>,
    /// The asset for each completed job.
    assets: Vec<GeneratedAsset>,
    /// The selected tray position, if any.
    selected: Option<usize>,
}

/// Holds completed [`GeneratedAsset`]s, ordered, with a selection.
#[derive(Default)]
pub struct ResultStore {
    inner: Mutex<ResultState>,
}

impl ResultStore {
    /// An empty result store. Wrap in `Arc` to share with job tasks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a completed asset; selects it if nothing is selected yet.
    pub(crate) fn put(&self, job: JobId, asset: GeneratedAsset) {
        let mut state = self.inner.lock();
        state.order.push(job);
        state.assets.push(asset);
        if state.selected.is_none() {
            state.selected = Some(state.assets.len() - 1);
        }
    }

    /// The number of completed results.
    pub fn len(&self) -> usize {
        self.inner.lock().assets.len()
    }

    /// Whether there are no results.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().assets.is_empty()
    }

    /// Selects the result at `index` (ignored if out of range).
    pub fn select(&self, index: usize) {
        let mut state = self.inner.lock();
        if index < state.assets.len() {
            state.selected = Some(index);
        }
    }

    /// The selected tray position, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.inner.lock().selected
    }

    /// A clone of the selected asset, if any.
    pub fn selected(&self) -> Option<GeneratedAsset> {
        let state = self.inner.lock();
        let index = state.selected?;
        state.assets.get(index).cloned()
    }

    /// The provenance of the result at `index`, without consuming it.
    pub fn meta(&self, index: usize) -> Option<GenerationProvenance> {
        let state = self.inner.lock();
        state.assets.get(index).map(|a| a.provenance.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::GenerationProvenance;

    fn asset(tag: u8) -> GeneratedAsset {
        GeneratedAsset {
            width: 1,
            height: 1,
            stride: 4,
            rgba: vec![tag, tag, tag, 255],
            provenance: GenerationProvenance {
                prompt: format!("p{tag}"),
                seed: u64::from(tag),
                provider_id: "mock".to_owned(),
                model: "m".to_owned(),
                created_unix_ms: 0,
            },
        }
    }

    #[test]
    fn put_selects_first_and_keeps_order() {
        let store = ResultStore::new();
        assert!(store.is_empty());
        store.put(JobId(0), asset(1));
        store.put(JobId(1), asset(2));
        assert_eq!(store.len(), 2);
        // First put becomes the selection; later puts do not steal it.
        assert_eq!(store.selected_index(), Some(0));
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(1));
    }

    #[test]
    fn select_changes_the_selection_and_meta_reads_without_consuming() {
        let store = ResultStore::new();
        store.put(JobId(0), asset(1));
        store.put(JobId(1), asset(2));
        store.select(1);
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(2));
        assert_eq!(store.meta(1).map(|m| m.seed), Some(2));
        // Still selectable and present after reading meta.
        assert_eq!(store.len(), 2);
        store.select(99); // out of range: ignored
        assert_eq!(store.selected_index(), Some(1));
    }
}
