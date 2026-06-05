//! The result store: completed generated assets, held until applied or discarded.
//!
//! Results enter here as transient job output (bible 17.2) and stay until the user
//! applies one (through a command) or discards it. This is the one genuinely
//! shared-across-threads piece — the job task writes, the UI lane reads — so it is
//! the sanctioned `Arc<Mutex<…>>` exception to the single-owner rule. The lock is
//! never held across an `.await`: every critical section is a quick read or write.

use parking_lot::Mutex;

use crate::generated::{GeneratedAnimation, GeneratedAsset, GeneratedResult, GenerationProvenance, ResultKind};
use crate::job::JobId;

#[derive(Default)]
struct ResultState {
    /// Completed jobs in arrival order; the index into this is the tray position.
    order: Vec<JobId>,
    /// The result for each completed job (a still sprite or an animation).
    results: Vec<GeneratedResult>,
    /// The selected tray position, if any.
    selected: Option<usize>,
}

/// Holds completed [`GeneratedResult`]s, ordered, with a selection. Both still
/// anchors and animations live in one ordered tray.
#[derive(Default)]
pub struct ResultStore {
    inner: Mutex<ResultState>,
}

impl ResultStore {
    /// An empty result store. Wrap in `Arc` to share with job tasks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a completed result; selects it if nothing is selected yet.
    pub(crate) fn put(&self, job: JobId, result: GeneratedResult) {
        let mut state = self.inner.lock();
        state.order.push(job);
        state.results.push(result);
        if state.selected.is_none() {
            state.selected = Some(state.results.len() - 1);
        }
    }

    /// The number of completed results.
    pub fn len(&self) -> usize {
        self.inner.lock().results.len()
    }

    /// Whether there are no results.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().results.is_empty()
    }

    /// Selects the result at `index` (ignored if out of range).
    pub fn select(&self, index: usize) {
        let mut state = self.inner.lock();
        if index < state.results.len() {
            state.selected = Some(index);
        }
    }

    /// Discards the result at `index`, removing it from the tray. Out-of-range
    /// indices are ignored. Returns whether a result was removed.
    ///
    /// Selection rule, since a discard renumbers every later tray position:
    /// - Discarding the selected result keeps the selection on the same tray slot,
    ///   which now holds what was the next result, clamped to the new last index;
    ///   the tray emptying clears the selection. Holding the slot rather than
    ///   dropping to none means the user keeps reviewing results after tossing one,
    ///   which is the point of a discard-and-move-on tray.
    /// - Discarding a result before the selected one shifts the selection down by
    ///   one so it still points at the same result, not its neighbor.
    /// - Discarding a result after the selected one leaves the selection untouched.
    pub fn discard(&self, index: usize) -> bool {
        let mut state = self.inner.lock();
        if index >= state.results.len() {
            return false;
        }
        state.order.remove(index);
        state.results.remove(index);
        state.selected = match state.selected {
            // Tray emptied: nothing to point at.
            _ if state.results.is_empty() => None,
            Some(sel) if sel == index => Some(sel.min(state.results.len() - 1)),
            // Earlier removal renumbers the selected slot down one.
            Some(sel) if sel > index => Some(sel - 1),
            other => other,
        };
        true
    }

    /// Discards every result and clears the selection. The tray returns to empty,
    /// matching a fresh [`ResultStore::new`].
    pub fn clear(&self) {
        let mut state = self.inner.lock();
        state.order.clear();
        state.results.clear();
        state.selected = None;
    }

    /// The selected tray position, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.inner.lock().selected
    }

    /// A clone of the selected result if it is a still sprite, else `None`.
    pub fn selected(&self) -> Option<GeneratedAsset> {
        let state = self.inner.lock();
        let index = state.selected?;
        match state.results.get(index)? {
            GeneratedResult::Sprite(asset) => Some(asset.clone()),
            GeneratedResult::Animation(_) => None,
        }
    }

    /// A clone of the selected result if it is an animation, else `None`.
    pub fn selected_animation(&self) -> Option<GeneratedAnimation> {
        let state = self.inner.lock();
        let index = state.selected?;
        match state.results.get(index)? {
            GeneratedResult::Animation(animation) => Some(animation.clone()),
            GeneratedResult::Sprite(_) => None,
        }
    }

    /// A clone of the still sprite at `index`, or `None` if absent or an animation.
    /// Used to take an anchor result as the reference image for an idle pass.
    pub fn asset_at(&self, index: usize) -> Option<GeneratedAsset> {
        let state = self.inner.lock();
        match state.results.get(index)? {
            GeneratedResult::Sprite(asset) => Some(asset.clone()),
            GeneratedResult::Animation(_) => None,
        }
    }

    /// The provenance of the result at `index`, without consuming it.
    pub fn meta(&self, index: usize) -> Option<GenerationProvenance> {
        let state = self.inner.lock();
        state.results.get(index).map(|r| r.provenance().clone())
    }

    /// A kind summary of each result in tray order, for the shell's read-only mirror.
    pub fn kinds_summary(&self) -> Vec<ResultKind> {
        self.inner.lock().results.iter().map(GeneratedResult::kind).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{GeneratedAnimation, GeneratedFrame, GenerationProvenance};
    use pixhaus_core::LoopMode;

    fn provenance(tag: u8) -> GenerationProvenance {
        GenerationProvenance {
            prompt: format!("p{tag}"),
            seed: u64::from(tag),
            provider_id: "mock".to_owned(),
            model: "m".to_owned(),
            created_unix_ms: 0,
        }
    }

    fn sprite(tag: u8) -> GeneratedResult {
        GeneratedResult::Sprite(GeneratedAsset {
            width: 1,
            height: 1,
            stride: 4,
            rgba: vec![tag, tag, tag, 255],
            provenance: provenance(tag),
        })
    }

    fn animation(tag: u8, frames: usize) -> GeneratedResult {
        GeneratedResult::Animation(GeneratedAnimation {
            frames: (0..frames)
                .map(|_| GeneratedFrame {
                    width: 1,
                    height: 1,
                    stride: 4,
                    rgba: vec![tag, tag, tag, 255],
                })
                .collect(),
            clip_name: "idle".to_owned(),
            fps: 12,
            loop_mode: LoopMode::Loop,
            provenance: provenance(tag),
        })
    }

    #[test]
    fn put_selects_first_and_keeps_order() {
        let store = ResultStore::new();
        assert!(store.is_empty());
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), sprite(2));
        assert_eq!(store.len(), 2);
        // First put becomes the selection; later puts do not steal it.
        assert_eq!(store.selected_index(), Some(0));
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(1));
    }

    #[test]
    fn select_changes_the_selection_and_meta_reads_without_consuming() {
        let store = ResultStore::new();
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), sprite(2));
        store.select(1);
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(2));
        assert_eq!(store.meta(1).map(|m| m.seed), Some(2));
        // Still selectable and present after reading meta.
        assert_eq!(store.len(), 2);
        store.select(99); // out of range: ignored
        assert_eq!(store.selected_index(), Some(1));
    }

    #[test]
    fn sprite_and_animation_accessors_are_kind_specific() {
        let store = ResultStore::new();
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), animation(2, 8));

        // The anchor is reachable as an asset, not as an animation.
        assert!(store.asset_at(0).is_some());
        store.select(0);
        assert!(store.selected().is_some(), "sprite selected as an asset");
        assert!(store.selected_animation().is_none(), "a sprite is not an animation");

        // The animation is reachable as an animation, not as an asset.
        store.select(1);
        assert!(store.selected().is_none(), "an animation is not a still asset");
        assert_eq!(store.selected_animation().map(|a| a.frames.len()), Some(8));
        assert!(store.asset_at(1).is_none(), "an animation is not an asset_at");
    }

    #[test]
    fn discard_of_selected_holds_the_slot_and_clamps_at_the_end() {
        let store = ResultStore::new();
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), sprite(2));
        store.put(JobId(2), sprite(3));
        store.select(1); // selecting the middle result (tag 2)

        // Discarding the selected slot keeps the selection there; it now holds tag 3.
        assert!(store.discard(1));
        assert_eq!(store.len(), 2);
        assert_eq!(store.selected_index(), Some(1));
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(3));

        // Discarding the now-last selected slot clamps the selection back one.
        assert!(store.discard(1));
        assert_eq!(store.selected_index(), Some(0));
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(1));

        // Discarding the final result empties the tray and clears the selection.
        assert!(store.discard(0));
        assert!(store.is_empty());
        assert_eq!(store.selected_index(), None);
    }

    #[test]
    fn discard_of_non_selected_keeps_selection_on_the_same_result() {
        let store = ResultStore::new();
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), sprite(2));
        store.put(JobId(2), sprite(3));
        store.select(2); // selecting tag 3 at index 2

        // Discarding an earlier result shifts the selection down to still point at tag 3.
        assert!(store.discard(0));
        assert_eq!(store.selected_index(), Some(1));
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(3));

        // Discarding a later result leaves the selection untouched.
        store.put(JobId(3), sprite(4)); // tray: [2, 3, 4], selection on 3 at index 1
        assert!(store.discard(2));
        assert_eq!(store.selected_index(), Some(1));
        assert_eq!(store.selected().map(|a| a.rgba[0]), Some(3));

        // An out-of-range discard is a no-op and reports false.
        assert!(!store.discard(99));
        assert_eq!(store.len(), 2);
        assert_eq!(store.selected_index(), Some(1));
    }

    #[test]
    fn clear_empties_the_tray_and_drops_the_selection() {
        let store = ResultStore::new();
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), animation(2, 8));
        store.select(1);

        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.selected_index(), None);
        assert_eq!(store.kinds_summary(), Vec::new());
    }

    #[test]
    fn kinds_summary_reports_each_result_kind_in_order() {
        let store = ResultStore::new();
        store.put(JobId(0), sprite(1));
        store.put(JobId(1), animation(2, 8));
        assert_eq!(store.kinds_summary(), vec![ResultKind::Sprite, ResultKind::Animation { frames: 8 }]);
    }
}
