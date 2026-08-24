//! crochet-core: the insertion-graph simulation engine.
//!
//! See ../../docs/crochet-context.md for the domain model this crate
//! implements (stitch anatomy, the insertion graph, shaping, elasticity,
//! and the geometric invariants), and ../../GOALS.md for the milestone
//! this crate is being built against.

pub mod geometry;
pub mod graph;
pub mod path;
pub mod relax;
pub mod stitch;
pub mod validate;
pub mod vec3;
