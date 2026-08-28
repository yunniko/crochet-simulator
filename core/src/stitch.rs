//! Stitch registry — see docs/crochet-context.md §3 and §3a.
//!
//! Every stitch is described by the same three-part recipe (pre-wraps,
//! insertion, draw-throughs); `ch` is the one stitch with no insertion at
//! all. The registry is open (§3a): new entries — textured/compound
//! stitches, other regional traditions — are added by registering a new
//! `StitchDef`, not by extending an enum or touching placement code.

use std::collections::HashMap;

/// Stable internal identifier for a stitch kind, independent of any
/// display language (UK/US/other — see docs/crochet-context.md §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StitchId(pub &'static str);

pub const CH: StitchId = StitchId("ch");
pub const SS: StitchId = StitchId("ss");
pub const DC: StitchId = StitchId("dc");
pub const HTR: StitchId = StitchId("htr");
pub const TR: StitchId = StitchId("tr");
pub const DTR: StitchId = StitchId("dtr");
pub const TRTR: StitchId = StitchId("trtr");
pub const QUAD_TR: StitchId = StitchId("quad_tr");
/// Magic ring / adjustable loop (docs §2, §5a, §4's foundations note): a
/// single loop of working yarn, not a run of chains — a genuinely
/// different construction from `ch`, not "ch with a different name." It
/// shares two *engine-level* properties with `ch` (no insertion step,
/// zero height, §3) because both are foundation anchors, which is why it
/// shares `has_insertion: false` below — that's a coincidence of how the
/// placement engine represents "a foundation point," not a claim about
/// real-world formation. Registered as its own kind so it can carry its
/// own capacity behaviour as a *target* (see `CapacityStyle`), which `ch`
/// does not share.
pub const MR: StitchId = StitchId("mr");
/// Starting chain (M9-era UI addition): the very first stitch of a
/// scheme, when the Owner opens with a chain rather than a magic ring.
/// Physically and geometrically identical to `ch` in every respect —
/// zero targets, zero height, lays out as a line — it exists as its own
/// registered kind purely so the editor can tell "the foundation-only
/// opening stitch" apart from an ordinary mid-scheme `ch` for tool-
/// availability purposes (only usable with zero stitches placed, like
/// `mr`, and its own presence alone doesn't yet unlock post stitches the
/// way an ordinary `ch` does — see `web/lib/tool-placement.ts`). None of
/// that distinction matters to placement/relaxation/validation, which is
/// exactly why it's a plain clone of `ch`'s definition below rather than
/// a new physical concept.
pub const START_CH: StitchId = StitchId("start_ch");

/// How a stitch behaves as an insertion **target** when several stitches
/// share it — see docs/crochet-context.md §5a. This is about the target's
/// own physical give, not the insertion-stiffness of whatever's worked
/// into it (`insertion_stiffness`, which is about the *inserting*
/// stitch's own kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityStyle {
    /// Comfortable capacity is small and roughly fixed — an ordinary
    /// stitch's own top loop physically only fits so much. Beyond
    /// capacity, extra siblings bulge out of the flat plane (a 3D wave)
    /// rather than crowd past each other in-plane.
    Fixed,
    /// Comfortable capacity grows without bound to keep spacing
    /// comfortable — the target opens wider to accommodate more (a chain,
    /// or a chain-space: "chain geometry is very elastic," per the
    /// Owner). No plateau, no waving.
    Elastic,
    /// A magic ring, pulled tight (the Owner's calibration, docs §5a):
    /// **3–5 stitches** cinch into a small radius (reads as a narrow,
    /// pointier 3D shape rather than a flat disc); **6–8** is the flat-
    /// circle sweet spot; **9+** can't open further and ripples into a
    /// wavy 3D circle; far beyond that, physically can't be tightened
    /// into one point at all regardless of geometry (yarn thickness) —
    /// this engine doesn't hard-block it, but the wave stops growing
    /// enough to keep avoiding collisions, so `crate::validate` will
    /// correctly flag it eventually. An MR left deliberately un-tightened
    /// should use `Elastic` instead (via `StitchInstance::
    /// capacity_override`) — this variant models the tightened case,
    /// which is the ordinary/default use of a magic ring.
    TightenedRing,
}

/// How the loops on the hook are cleared once the pre-wraps and the
/// inserted loop are on it. See docs/crochet-context.md §3 step 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawThrough {
    /// `ss`: one draw-through clearing the stitch loop and the working
    /// loop together — zero net height.
    SlipClear,
    /// `dc`: one draw-through, pulling through both loops on the hook.
    Single,
    /// `htr`: one draw-through, pulling through all loops on the hook at
    /// once (this is what makes it shorter than `tr` despite the same
    /// one pre-wrap).
    AllAtOnce,
    /// `tr` and taller: "yarn over, pull through 2" repeated until one
    /// loop remains.
    Repeated2,
}

#[derive(Debug, Clone)]
pub struct StitchDef {
    pub id: StitchId,
    /// Yarn-overs before inserting the hook. 0 for `dc`, 1 for `htr`/`tr`,
    /// 2 for `dtr`, etc. Meaningless (left 0) when `has_insertion` is false.
    pub pre_wraps: u32,
    /// False for `ch` and `mr`: neither has an insertion step at all,
    /// both are formed/started purely from the working loop or a drawn-up
    /// loop (docs §3, §4, §8 invariant 2).
    pub has_insertion: bool,
    pub draw_through: DrawThrough,
    /// How this stitch behaves as a *target* for other stitches — see
    /// `CapacityStyle`.
    pub capacity_style: CapacityStyle,
    /// True only for `ch`: a zero-target stitch that lays itself out as
    /// a step forward from wherever the thread left off, forming a line.
    /// `mr` is also zero-target but is a *point* anchor (docs §5a — a
    /// ring, not a line), so it must stay false here even though both
    /// share `has_insertion: false`. Meaningless when a stitch has
    /// targets (only ever read for the empty-targets placement case).
    pub lays_out_as_line: bool,
}

impl StitchDef {
    /// Abstract stitch height. Driven by pre-wrap count, but not just a
    /// flat function of it: how the loops are cleared matters too (docs
    /// §3) — `htr` pre-wraps once like `tr` but clears everything in one
    /// motion instead of stepwise, so it comes out shorter than `tr`
    /// despite the same pre-wrap count.
    pub fn height(&self) -> f64 {
        if !self.has_insertion {
            return 0.0;
        }
        match self.draw_through {
            DrawThrough::SlipClear => 0.0,
            DrawThrough::Single => 1.0,
            // One clearing motion regardless of pre-wraps, but each
            // pre-wrap still holds some extra yarn vertically before that
            // motion happens — half credit relative to a full Repeated2 stage.
            DrawThrough::AllAtOnce => 1.0 + 0.5 * self.pre_wraps as f64,
            // One "yarn over, pull through 2" stage per pre-wrap, plus the
            // stage that clears the originally-inserted loop.
            DrawThrough::Repeated2 => self.pre_wraps as f64 + 1.0,
        }
    }

    /// How stiffly this stitch resists its insertion point(s) deviating
    /// from rest length — see docs/crochet-context.md §6: elasticity is a
    /// property of stitch topology, not a separate yarn-material number.
    /// Dense, short-draw-through stitches are stiff (low give); tall,
    /// multi-stage stitches are soft (more give); `ch` is the loosest
    /// connection of all. Values are relative (used as spring constants
    /// in `crate::relax`), not calibrated to any physical unit.
    pub fn insertion_stiffness(&self) -> f64 {
        if !self.has_insertion {
            return 0.15; // ch
        }
        match self.draw_through {
            DrawThrough::SlipClear => 0.9, // ss: cinches tight, minimal give
            DrawThrough::Single => 0.8,    // dc: dense, low give
            DrawThrough::AllAtOnce => 0.5, // htr: medium
            DrawThrough::Repeated2 => {
                // tr and taller: more pre-wraps -> looser insertion, more give.
                (0.5 - 0.08 * self.pre_wraps as f64).max(0.1)
            }
        }
    }

    /// Number of points along this stitch's own yarn path, used to give a
    /// taller stitch a proportionally more subdivided path (§3: more
    /// pre-wraps -> more yarn held vertically before the first
    /// draw-through). Chains and slip stitches get the minimum, 1 segment.
    pub fn path_segments(&self) -> u32 {
        if !self.has_insertion {
            1
        } else {
            self.pre_wraps + 1
        }
    }
}

pub struct StitchRegistry {
    defs: HashMap<StitchId, StitchDef>,
}

impl StitchRegistry {
    pub fn empty() -> Self {
        StitchRegistry {
            defs: HashMap::new(),
        }
    }

    /// Seeds the registry with the basic UK stitch ladder from
    /// docs/crochet-context.md §1. Other stitch traditions/textured
    /// stitches are deliberately not seeded here (§3a) — register them
    /// separately when that milestone is reached.
    pub fn with_uk_basics() -> Self {
        let mut reg = Self::empty();
        reg.register(StitchDef {
            id: CH,
            pre_wraps: 0,
            has_insertion: false,
            draw_through: DrawThrough::Single,
            capacity_style: CapacityStyle::Elastic,
            lays_out_as_line: true,
        });
        reg.register(StitchDef {
            id: START_CH,
            pre_wraps: 0,
            has_insertion: false,
            draw_through: DrawThrough::Single,
            capacity_style: CapacityStyle::Elastic,
            lays_out_as_line: true,
        });
        reg.register(StitchDef {
            id: MR,
            pre_wraps: 0,
            has_insertion: false,
            draw_through: DrawThrough::Single,
            // Tightened is the ordinary/default use of a magic ring
            // (that's the point of it vs. a chain ring) — see
            // `CapacityStyle::TightenedRing`.
            capacity_style: CapacityStyle::TightenedRing,
            // A point anchor, not a line (docs §5a) — unlike `ch`.
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: SS,
            pre_wraps: 0,
            has_insertion: true,
            draw_through: DrawThrough::SlipClear,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: DC,
            pre_wraps: 0,
            has_insertion: true,
            draw_through: DrawThrough::Single,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: HTR,
            pre_wraps: 1,
            has_insertion: true,
            draw_through: DrawThrough::AllAtOnce,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: TR,
            pre_wraps: 1,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: DTR,
            pre_wraps: 2,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: TRTR,
            pre_wraps: 3,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg.register(StitchDef {
            id: QUAD_TR,
            pre_wraps: 4,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        reg
    }

    /// The extensibility point referenced throughout docs/crochet-context.md
    /// §3a: adding a stitch later is a call to this, not a redesign.
    pub fn register(&mut self, def: StitchDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: StitchId) -> Option<&StitchDef> {
        self.defs.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ch_has_no_insertion_and_zero_height() {
        let reg = StitchRegistry::with_uk_basics();
        let ch = reg.get(CH).unwrap();
        assert!(!ch.has_insertion);
        assert_eq!(ch.height(), 0.0);
    }

    #[test]
    fn ss_has_insertion_but_zero_height() {
        let reg = StitchRegistry::with_uk_basics();
        let ss = reg.get(SS).unwrap();
        assert!(ss.has_insertion);
        assert_eq!(ss.height(), 0.0);
    }

    #[test]
    fn stitch_height_increases_with_pre_wraps() {
        let reg = StitchRegistry::with_uk_basics();
        let heights: Vec<f64> = [DC, HTR, TR, DTR, TRTR, QUAD_TR]
            .iter()
            .map(|id| reg.get(*id).unwrap().height())
            .collect();
        for pair in heights.windows(2) {
            assert!(
                pair[1] > pair[0],
                "expected strictly increasing heights: {:?}",
                heights
            );
        }
    }

    #[test]
    fn htr_and_tr_share_pre_wrap_count_but_differ_in_draw_through() {
        let reg = StitchRegistry::with_uk_basics();
        let htr = reg.get(HTR).unwrap();
        let tr = reg.get(TR).unwrap();
        assert_eq!(htr.pre_wraps, tr.pre_wraps);
        assert_ne!(htr.draw_through, tr.draw_through);
        // htr's single all-at-once draw-through makes it shorter than tr's.
        assert!(htr.height() < tr.height());
    }

    #[test]
    fn insertion_stiffness_decreases_as_stitches_get_taller() {
        let reg = StitchRegistry::with_uk_basics();
        let stiffnesses: Vec<f64> = [DC, HTR, TR, DTR, TRTR, QUAD_TR]
            .iter()
            .map(|id| reg.get(*id).unwrap().insertion_stiffness())
            .collect();
        for pair in stiffnesses.windows(2) {
            assert!(
                pair[1] < pair[0],
                "expected strictly decreasing stiffness (more give as stitches get taller): {:?}",
                stiffnesses
            );
        }
        // ch is the loosest connection of all.
        let ch_stiffness = reg.get(CH).unwrap().insertion_stiffness();
        assert!(ch_stiffness < *stiffnesses.last().unwrap());
    }

    #[test]
    fn registry_is_extensible() {
        let mut reg = StitchRegistry::with_uk_basics();
        let custom = StitchId("bobble");
        reg.register(StitchDef {
            id: custom,
            pre_wraps: 1,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
            capacity_style: CapacityStyle::Fixed,
            lays_out_as_line: false,
        });
        assert!(reg.get(custom).is_some());
    }

    #[test]
    fn mr_has_no_insertion_and_defaults_to_tightened_ring() {
        let reg = StitchRegistry::with_uk_basics();
        let mr = reg.get(MR).unwrap();
        assert!(!mr.has_insertion);
        assert_eq!(mr.height(), 0.0);
        assert_eq!(mr.capacity_style, CapacityStyle::TightenedRing);
    }

    #[test]
    fn ordinary_stitches_default_to_fixed_capacity() {
        let reg = StitchRegistry::with_uk_basics();
        for id in [SS, DC, HTR, TR, DTR, TRTR, QUAD_TR] {
            assert_eq!(reg.get(id).unwrap().capacity_style, CapacityStyle::Fixed);
        }
    }

    #[test]
    fn ch_is_elastic() {
        let reg = StitchRegistry::with_uk_basics();
        assert_eq!(reg.get(CH).unwrap().capacity_style, CapacityStyle::Elastic);
    }

    #[test]
    fn start_ch_is_a_physical_clone_of_ch() {
        // See START_CH's own doc comment: it's registered separately only
        // for the editor's tool-availability rules to key off of — every
        // placement/geometry-relevant property must match `ch` exactly.
        let reg = StitchRegistry::with_uk_basics();
        let ch = reg.get(CH).unwrap();
        let start_ch = reg.get(START_CH).unwrap();
        assert_eq!(start_ch.has_insertion, ch.has_insertion);
        assert_eq!(start_ch.height(), ch.height());
        assert_eq!(start_ch.capacity_style, ch.capacity_style);
        assert_eq!(start_ch.lays_out_as_line, ch.lays_out_as_line);
        assert_eq!(start_ch.insertion_stiffness(), ch.insertion_stiffness());
    }
}
