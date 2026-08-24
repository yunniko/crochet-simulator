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
    /// False only for `ch`: a chain has no insertion step at all, it's
    /// formed purely from the working loop (docs §3, §4, §8 invariant 2).
    pub has_insertion: bool,
    pub draw_through: DrawThrough,
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
        });
        reg.register(StitchDef {
            id: SS,
            pre_wraps: 0,
            has_insertion: true,
            draw_through: DrawThrough::SlipClear,
        });
        reg.register(StitchDef {
            id: DC,
            pre_wraps: 0,
            has_insertion: true,
            draw_through: DrawThrough::Single,
        });
        reg.register(StitchDef {
            id: HTR,
            pre_wraps: 1,
            has_insertion: true,
            draw_through: DrawThrough::AllAtOnce,
        });
        reg.register(StitchDef {
            id: TR,
            pre_wraps: 1,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
        });
        reg.register(StitchDef {
            id: DTR,
            pre_wraps: 2,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
        });
        reg.register(StitchDef {
            id: TRTR,
            pre_wraps: 3,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
        });
        reg.register(StitchDef {
            id: QUAD_TR,
            pre_wraps: 4,
            has_insertion: true,
            draw_through: DrawThrough::Repeated2,
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
        });
        assert!(reg.get(custom).is_some());
    }
}
