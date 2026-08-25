//! The insertion graph — see docs/crochet-context.md §4, §4a, §5.
//!
//! A scheme is a list of threads (§4a — deferred multi-thread support,
//! but the type is a list from day one). Each thread is a working-order
//! sequence of stitch instances; a stitch's "connection points" are its
//! `targets` (§5), not a row/round relationship — rows/rounds are not
//! modelled here at all, by design (§4, HANDOVER D4).

use crate::stitch::{CapacityStyle, StitchId};

/// Which loop(s) of the target this stitch's hook insertion goes through.
/// Irrelevant for `ch` (no insertion at all).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopTarget {
    #[default]
    Both,
    FrontOnly,
    BackOnly,
    FrontPost,
    BackPost,
}

/// A reference to an earlier stitch instance, anywhere in the scheme.
/// Cross-thread refs (`thread` != the referencing stitch's own thread)
/// are the §4a "crochet join" case — unused until multi-thread schemes
/// are built, but the type already allows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StitchRef {
    pub thread: usize,
    pub index: usize,
}

impl StitchRef {
    pub fn new(thread: usize, index: usize) -> Self {
        StitchRef { thread, index }
    }
}

#[derive(Debug, Clone)]
pub struct StitchInstance {
    pub kind: StitchId,
    pub loop_target: LoopTarget,
    /// Insertion-target references (§4/§5): empty for `ch`, one for a
    /// plain stitch, shared with siblings for an increase, several for a
    /// decrease. No special case for spike stitches or freeform work —
    /// a target is just a `StitchRef`, wherever it points (§8 invariant 2).
    pub targets: Vec<StitchRef>,
    /// Overrides the registry-default `CapacityStyle` for this stitch
    /// **when it is used as another stitch's target** (§5a) — e.g. mark a
    /// specific magic-ring instance as deliberately left open
    /// (`Some(Elastic)`) instead of the tightened default. `None` = use
    /// the registry default for `kind`.
    pub capacity_override: Option<CapacityStyle>,
}

impl StitchInstance {
    pub fn new(kind: StitchId, targets: Vec<StitchRef>) -> Self {
        StitchInstance {
            kind,
            loop_target: LoopTarget::default(),
            targets,
            capacity_override: None,
        }
    }

    pub fn with_loop_target(mut self, loop_target: LoopTarget) -> Self {
        self.loop_target = loop_target;
        self
    }

    pub fn with_capacity_override(mut self, style: CapacityStyle) -> Self {
        self.capacity_override = Some(style);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Thread {
    pub stitches: Vec<StitchInstance>,
}

impl Thread {
    pub fn new() -> Self {
        Thread {
            stitches: Vec::new(),
        }
    }
}

/// The top-level object: a list of one-or-more threads (§4a, HANDOVER D9).
/// Only ever contains one thread until multi-thread schemes (deferred)
/// are built, but the type is never a singleton.
#[derive(Debug, Clone, Default)]
pub struct Scheme {
    pub threads: Vec<Thread>,
}

impl Scheme {
    pub fn new() -> Self {
        Scheme {
            threads: Vec::new(),
        }
    }

    pub fn add_thread(&mut self, thread: Thread) -> usize {
        self.threads.push(thread);
        self.threads.len() - 1
    }

    pub fn get(&self, r: StitchRef) -> Option<&StitchInstance> {
        self.threads.get(r.thread)?.stitches.get(r.index)
    }

    /// Total stitch instances across every thread (docs §8 invariant 3's
    /// "walk the graph and sum" self-check, at its simplest).
    pub fn total_stitch_count(&self) -> usize {
        self.threads.iter().map(|t| t.stitches.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stitch::{CH, DC};

    #[test]
    fn scheme_is_a_list_of_threads_not_a_singleton() {
        let scheme = Scheme::new();
        assert_eq!(scheme.threads.len(), 0);
        // The type permits more than one thread even though M1 never
        // populates more than one — see docs §4a / HANDOVER D9.
    }

    #[test]
    fn chain_instance_has_no_targets() {
        let ch = StitchInstance::new(CH, vec![]);
        assert!(ch.targets.is_empty());
    }

    #[test]
    fn stitch_ref_can_point_across_threads() {
        let r = StitchRef::new(1, 3);
        assert_eq!(r.thread, 1);
        assert_eq!(r.index, 3);
    }

    #[test]
    fn total_stitch_count_sums_across_threads() {
        let mut scheme = Scheme::new();
        let mut t0 = Thread::new();
        t0.stitches.push(StitchInstance::new(CH, vec![]));
        t0.stitches
            .push(StitchInstance::new(DC, vec![StitchRef::new(0, 0)]));
        scheme.add_thread(t0);
        assert_eq!(scheme.total_stitch_count(), 2);
    }
}
