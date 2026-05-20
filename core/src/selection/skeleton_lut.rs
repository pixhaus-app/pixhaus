//! Generated 256-entry classification LUT for the 8-neighbour
//! signature used by the gap-closing magic wand (see `core/build.rs`).
//!
//! Adapted from `OpenToonz` `toonz/sources/toonzlib/skeletonlut.h` and
//! the consumer in `toonz/sources/common/trop/tautoclose.cpp` under
//! BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
//!
//! # Bit ordering
//!
//! The neighbour code packs the 8 neighbours into a single byte using
//! the same layout as the `OpenToonz` `neighboursCode` helper
//! (`tautoclose.cpp:54-59`):
//!
//! - bit 0: SW
//! - bit 1: S
//! - bit 2: SE
//! - bit 3: W
//! - bit 4: E
//! - bit 5: NW
//! - bit 6: N
//! - bit 7: NE
//!
//! Each entry in `SKELETON_LUT` answers "given this neighbour code,
//! what kind of skeleton pixel is this?". The classifier walks every
//! ink pixel once and indexes this table to bucket it as `Isolated`,
//! `Endpoint`, `Border`, `Branch`, or `Interior`. The autoclose
//! pre-pass only cares about `Endpoint`.

/// Classification bucket for an 8-neighbour signature.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Classification {
    /// Zero ink neighbours.
    Isolated,
    /// One neighbour, or two non-adjacent neighbours — a stroke
    /// terminus.
    Endpoint,
    /// Two adjacent neighbours, or 3-4 neighbours in fewer than three
    /// runs — a stroke edge pixel.
    Border,
    /// Three or more disjoint runs of neighbours — a junction.
    Branch,
    /// Five or more set neighbours — fully enclosed.
    Interior,
}

include!(concat!(env!("OUT_DIR"), "/skeleton_lut_table.rs"));

#[cfg(test)]
mod tests {
    use super::{Classification, SKELETON_LUT};

    #[test]
    fn isolated_classified_correctly() {
        assert_eq!(SKELETON_LUT[0], Classification::Isolated);
    }

    #[test]
    fn single_neighbour_is_endpoint() {
        assert_eq!(SKELETON_LUT[1], Classification::Endpoint);
        assert_eq!(SKELETON_LUT[2], Classification::Endpoint);
        assert_eq!(SKELETON_LUT[128], Classification::Endpoint);
    }

    #[test]
    fn adjacent_pair_is_border() {
        // Bits 0 and 1 (SW + S) are adjacent in the ring.
        assert_eq!(SKELETON_LUT[0b0000_0011], Classification::Border);
        // Bits 7 and 0 wrap around (NE + SW), also adjacent.
        assert_eq!(SKELETON_LUT[0b1000_0001], Classification::Border);
    }

    #[test]
    fn opposing_pair_is_endpoint() {
        // Bits 0 and 4 are diametrically opposite in the ring — counts
        // as a single straight stroke passing through this pixel, so
        // each end registers as an endpoint here.
        assert_eq!(SKELETON_LUT[0b0001_0001], Classification::Endpoint);
    }

    #[test]
    fn fully_surrounded_is_interior() {
        assert_eq!(SKELETON_LUT[0b1111_1111], Classification::Interior);
    }

    #[test]
    fn three_disjoint_runs_is_branch() {
        // Bits 0, 3, and 6 — three single bits with non-adjacent gaps.
        // Disjoint runs = 3 → branch.
        let code: u8 = (1 << 0) | (1 << 3) | (1 << 6);
        assert_eq!(SKELETON_LUT[code as usize], Classification::Branch);
    }

    #[test]
    fn adjacent_triple_is_border() {
        // Bits 0, 1, 2 form a single contiguous arc — one run only,
        // so the pixel sits on a thick stroke edge.
        let code: u8 = 0b0000_0111;
        assert_eq!(SKELETON_LUT[code as usize], Classification::Border);
    }
}
