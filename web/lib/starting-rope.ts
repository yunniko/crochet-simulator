import type { WireStitch } from "@/lib/stitch-kinds";

// The app's default starting scene (2026-08-30, Owner-directed): "on
// application start there was a long bendy rope present" — replacing both
// the old preset-button row and the old empty-canvas stub. Per the Owner's
// standing instruction ("nothing decorative... do not use shortcuts unless
// directly requested"), the bend is not drawn — it's real: this is an
// ordinary chain (`ch`) run through the exact same core placement/
// relaxation pipeline as any hand-built scheme, with one `ss` partway along
// pulling the chain back to an earlier point in itself. That's the same,
// already-proven mechanism M9 uses to close a chain into a genuine
// non-self-intersecting ring via real Discrete Elastic Rod bending physics
// (`core/src/relax.rs`) — applied here at a longer, asymmetric scale (a
// loop partway along, with a free tail continuing past it) so the result
// reads as a loosely coiled length of yarn with a trailing end, not a tight
// closed circle.
const LEAD_IN_LENGTH = 24;
const TAIL_LENGTH = 6;

export const STARTING_ROPE: WireStitch[] = [
  ...Array.from({ length: LEAD_IN_LENGTH }, () => ({ kind: "ch" as const, targets: [] })),
  { kind: "ss" as const, targets: [0] },
  ...Array.from({ length: TAIL_LENGTH }, () => ({ kind: "ch" as const, targets: [] })),
];
