# Crochet context — terminology & rules (UK/GB terms)

Domain reference for the crochet-sim engine and UI. **The app's canonical/
internal vocabulary is UK/GB terms** (this document). This matters because
UK and US crochet terminology use the *same words for different stitches*
(not just different words for the same stitch) — see the table below.

**Decision (2026-08-24, Owner):** stitch-name recognition should eventually
understand **multiple languages/naming systems**, not just US↔UK — e.g.
German, French, Japanese, Russian conventions. This doesn't need to ship
early, but it does mean the stitch registry (§3a) must key every stitch by
a stable internal ID from the start, with each naming system (UK, US,
other languages) as a separate localized-name mapping layer on top — never
name things in code by their UK abbreviation directly, and never hard-code
a single US↔UK table as if that were the whole problem.

**Confidence note (per Company standards on research/analysis):** this is
compiled from general, widely-consistent UK crochet convention (the terms
and construction rules below agree across mainstream UK publications and
yarn-craft sites such as Deramores, LoveCrafts/Let's Knit, and the Craft
Yarn Council's standard stitch definitions), not transcribed from one single
cited source. It has **not** been checked against a specific canonical UK
style guide line-by-line. Treat the terminology table as reliable; treat
edge-case construction details (marked ⚠ below) as needing a real
crochet-literate review before the engine encodes them as hard rules —
flag to the Owner rather than assuming.

## 1. Terminology — basic stitches (UK term = engine's canonical name)

| UK term | Abbrev. | US equivalent (same stitch, different name) | What it is |
|---|---|---|---|
| Chain | ch | Chain (ch) | Pull a new loop through the loop on the hook. The base unit; foundation chains and turning chains are built from these. |
| Slip stitch | ss / sl st | Slip stitch (sl st) | Insert hook, yarn over, pull through both the stitch *and* the loop on hook in one motion. Zero height — used to join, move position, or make a flat seam; not a "built up" stitch. |
| Double crochet | dc | **Single crochet (sc)** | Insert hook, yarn over, pull up a loop (2 loops on hook), yarn over, pull through both loops. Shortest "real" stitch — dense, firm fabric. |
| Half treble (crochet) | htr | **Half double crochet (hdc)** | Yarn over *before* inserting hook, insert, pull up a loop (3 loops on hook), yarn over, pull through all 3 loops at once. |
| Treble (crochet) | tr | **Double crochet (dc)** | Yarn over, insert, pull up a loop (3 loops on hook), [yarn over, pull through 2] twice. Taller, drapier fabric than dc/htr. |
| Double treble (crochet) | dtr | **Treble (tr)** | Yarn over twice before inserting (4 loops on hook after the pull-up), then [yarn over, pull through 2] three times. |
| Triple treble (crochet) | trtr / tr tr | Double treble (dtr) | Yarn over three times before inserting, [yarn over, pull through 2] four times. |
| Quadruple treble | quad tr | Triple treble (trtr) | Yarn over four times before inserting, [yarn over, pull through 2] five times. Rare outside lace/specialty work. |

**⚠ This is the single most important disambiguation in the whole
project**: US "single crochet" ≠ UK "single crochet" as a colloquial
shorthand — US patterns use "sc" for the shortest stitch, UK patterns call
that same stitch "dc." Every stitch name above shifts by one rung on the
US/UK ladder. Per the multi-language decision above, this needs to be one
instance of a general localized-name → canonical-stitch-ID mapping, not a
one-off US↔UK special case in code.

## 2. Other core terms

| Term | Abbrev. | Meaning |
|---|---|---|
| Yarn over hook | yoh | Wrap working yarn over the hook before drawing it through a loop/stitch. |
| Working loop | — | The single loop always left on the hook between stitches — the "live" end of the thread. |
| Stitch | st | One completed unit of the fabric; also used generically ("into next st"). |
| Foundation chain | — | The starting chain a piece is built from, when not starting from a ring. |
| Turning chain | t-ch | Chain(s) made to bring the hook up to the working height of the next stitch after a turn — see §4. |
| Magic ring / magic circle | MR | An adjustable starting loop (yarn wrapped into a ring, first stitches worked into it, then pulled closed) used to start a piece from a center point without a visible hole — very common start for amigurumi and flat circles/motifs. |
| Front loop / back loop | FLO / BLO | The two loops that make up the "V" at the top of every stitch. Normally both are worked into; working into only one changes texture and leaves the other as a visible ridge. |
| Front post / back post | FPtr, BPdc, etc. | Inserting the hook around the vertical **post** (body) of an earlier stitch, from front or back, instead of into its top loops. Produces raised, cable-like texture; the thread bypasses the top-loop insertion points entirely and reaches further back than the immediately preceding stitch. |
| Right side / wrong side | RS / WS | The face of the fabric meant to show / not show. Matters for post-stitch and colourwork direction, not for the base geometry. |
| Round | rnd | A closed loop of stitches worked back to (near) its own start, rather than a flat back-and-forth pass (tubes, circles, motifs, amigurumi). A *pattern-writing* grouping, not a structural unit the engine relies on — see §4. |
| Row | — | A flat, back-and-forth pass; work is turned at the end of each one. Same caveat as Round — see §4. |
| Increase | inc | Working 2 or more stitches into a single earlier stitch — widens the fabric. See §5. |
| Decrease | dec (e.g. dc2tog) | Working a single stitch that draws together 2+ earlier stitches — narrows the fabric. See §5. |
| Cluster / shell / bobble / popcorn / puff | — | Named groups of stitches worked into one point or space, left partially joined at the top — textured stitch groups, not new primitive stitches. Out of scope for the earliest milestones (confirmed by Owner, 2026-08-24) but the stitch registry (§3a) must let these be added later without a redesign. |
| Spike stitch | — | A stitch worked into an earlier point than the one immediately preceding it in working order — thread deliberately spans a gap. Under the insertion-graph model (§4) this is not a special case, just an insertion target further back than usual. |
| Fasten off | FO | Cut yarn, pull tail through the final working loop to secure it — ends a piece or section. |
| Join | — | Connecting the end of a round back to its start (commonly a slip stitch into the first stitch), or attaching a new piece/colour. |
| Gauge / tension | — | Stitch/row count per fixed measurement (e.g. "14 sts x 16 rows = 10cm") — depends on hook size, yarn weight, and the maker's hand tension. **Confirmed by Owner (2026-08-24): out of scope for now, may be revisited later** — the engine can assume idealised uniform stitch geometry until then. Distinct from elasticity (§6), which *is* in scope. |
| Blocking | — | Wetting/steaming and shaping a finished piece to set stitches into their final size/shape. Post-fabrication, out of scope for the geometry engine. |
| Hook size | mm | Crochet hooks are sized by shaft diameter in mm (UK sometimes also uses an old letter/number system, e.g. "4.00mm / UK 8" — always prefer the mm figure, the legacy letter scale is not consistent between old UK and US charts). |
| Yarn weight | ply / weight | Thickness category of yarn (UK commonly: 4-ply, DK — double knitting, Aran, Chunky, Super Chunky). Determines realistic stitch/thread scale; the engine can treat it as a single "thread diameter" parameter. |

## 3. Stitch anatomy — the general rule

Every UK stitch from `dc` up to `quad tr` is built from the same three-part
recipe, which is the thing the engine should actually encode (rather than
hard-coding each stitch name as an unrelated case):

1. **Pre-wraps.** Yarn over the hook *N* times before inserting, where
   *N* = 0 for `dc`, 1 for `htr`/`tr`, 2 for `dtr`, 3 for `trtr`, 4 for
   `quad tr`. (`htr` and `tr` both pre-wrap once — they differ only in
   step 3.)
2. **Insert & pull up a loop.** Insert the hook into the target stitch
   (normally both top loops — see FLO/BLO above), yarn over, pull a loop
   back through to the working side. This adds one loop to the hook, on
   top of the pre-wraps plus the original working loop.
3. **Draw-throughs.** Reduce the loops on the hook back to one:
   - `dc`: one draw-through, pulling through both loops on the hook.
   - `htr`: one draw-through, pulling through **all three** loops on the
     hook at once (this is what makes it shorter than `tr` despite having
     the same one pre-wrap).
   - `tr` and taller: repeated "yarn over, pull through **2** loops"
     until one loop remains — 2 repeats for `tr`, 3 for `dtr`, 4 for
     `trtr`, 5 for `quad tr`.

Stitch **height** is a direct function of pre-wrap count: more pre-wraps →
more yarn held vertically before the first draw-through → taller stitch and
looser fabric. This is the natural parameter for the engine's yarn-path
length/height calculation.

`ss` doesn't fit this recipe — it has zero pre-wraps and a single
draw-through that clears both the stitch loop and the working loop in one
motion, producing no net height.

**`ch` doesn't fit this recipe either, more fundamentally: it has zero
insertion targets.** Step 2 never happens for a chain — it's formed purely
by pulling a new loop through the current working loop, with nothing
inserted into and nothing else drawn through. **This is a fixed fact about
`ch`, true everywhere it appears — foundation chain, turning chain, a
chain within a row, a chain making a chain-space — there is no variant of
"chain" that has a target.** See §4 for why this means turning chains need
no special handling at all.

## 3a. The stitch set is a registry, not a fixed enum

**Confirmed by Owner (2026-08-24):** textured/compound stitches (clusters,
shells, bobbles, popcorns, puffs) and other regional stitch traditions can
stay out of scope for the earliest milestones, **but the engine must be
able to add them later without redesigning the core model.** Concretely:

- Every stitch type is an entry in an open registry, identified by a
  stable internal ID (not a UK abbreviation used as a key — see the
  multi-language decision above).
- A basic stitch's entry is the §3 recipe (pre-wrap count + draw-through
  pattern) parametrised, not a distinct code path per stitch name.
- A compound/textured stitch's entry is a **composition** of simpler
  stitches/insertions sharing a start or end point (e.g. a shell = several
  `tr` into one insertion point, a bobble = several incomplete `tr` drawn
  together at the top) — the same primitives as §3 and §5, combined, not a
  new geometric primitive.
- Adding a new stitch later (a new region's stitch, or a textured one)
  should mean registering a new entry, not touching the core simulation
  loop.

## 4. Core simulation model: an insertion graph, not rows

**Correction from the original draft of this document, per Owner
(2026-08-24): rows/rounds are not real objects in the simulation.** The
original version of this document modelled "row" and "round" as structural
units the engine reasons about directly (turning chains between rows,
insertion always into "the previous row," etc.). That's wrong for the
engine's core model, for a specific reason: **freehand/freeform crochet,
hyperbolic crochet, and other exotic styles genuinely don't work in rows**
— stitches can be worked into any earlier point in the piece, increase
rates can vary continuously rather than in discrete round-by-round steps,
and there is no consistent "previous row" to insert into. A row-based core
model would need special-casing (or an outright rewrite) to support these,
which defeats the point of building it as a general simulator.

**The actual model:** the piece is a single continuous thread, represented
as an ordered sequence of stitch instances in **working order** (the order
the maker's hook actually produces them — this ordering always exists,
since the yarn is one continuous strand). Each stitch instance carries:

- Its stitch-registry type (§3a) and any parameters (e.g. which loop(s) it
  targets — both top loops, FLO, BLO, or a post).
- One or more **insertion-target references** to earlier stitch instance(s)
  it draws its loop(s) from (see §5) — a plain stitch has exactly one
  target, an increase's siblings share a target, a decrease has multiple
  targets, a spike stitch's target is simply further back in working order
  than usual, and freeform/hyperbolic work's targets are whatever the
  designer points them at, with no constraint that they come from "the row
  below."

This is a graph, not a lattice: working-order gives a path (the yarn
itself), and insertion-target references are extra edges on top of it back
to earlier points in that same path. **Rows and rounds are a derived,
optional grouping on top of this graph** — useful for generating
human-readable pattern text (§7), showing gauge/row-counter UI, or
detecting "this happens to be conventional row-based construction" for
display purposes — but the simulation core never requires a stitch to
belong to one, and never assumes the previous stitch in working order is
also the correct default insertion target (it usually is, for
conventional row/round work, but that's a property of *what the designer
built*, not a rule the engine enforces).

**Foundations** (starting a piece) fit the same model, though the two
common starts are physically quite different constructions and shouldn't
be read as the same thing wearing different names: a **foundation chain**
is a run of several `ch` stitch instances in working order (each with no
insertion target of its own, §3), while a **magic ring** is *not* made of
chains at all — it's a single loop formed directly from the working yarn
(an adjustable/slip loop), with stitches worked around and into that one
loop before its tail is pulled to close it. In the model this shows up as
a real structural difference, not just a labelling one: a foundation
chain is several `ch` instances; a magic ring is exactly **one** `mr`
instance (§5a) that many stitches then share as a target. The two engine-
level properties `ch` and `mr` happen to have in common — no insertion
step, zero height (§3, §5a) — are a coincidence of how the placement
engine represents "a foundation point," not evidence they're the same
technique in reality.

**Turning chains are just chains — not a special case (correction, Owner,
2026-08-24).** A turning chain is structurally identical to any other
`ch` instance: zero insertion targets of its own (§3), sitting in working
order like any stitch. There is nothing about *being a turning chain* that
the engine needs to know or handle specially. The only thing that varies
by convention is an ordinary property of the *next* stitch worked after
the turn: which earlier point it chooses as its own insertion target
(possibly the turning chain's top, possibly the top of the stitch below
it, depending on stitch height and pattern convention) — and that's
already fully covered by the general insertion-target mechanism above, not
a rule about chains.

## 4a. Multiple threads and joins (deferred)

**Decision (2026-08-24, Owner):** a scheme should eventually support
**more than one thread/starting point, connected together later** —
confirmed out of scope for now, but must be addable without a redesign.
Two real crochet techniques motivate this directly:

- **Irish crochet**: individual motifs (flowers, leaves) are each worked
  from their own separate starting point/thread, then assembled — either
  joined *as you go* (a later motif's stitches are worked through an
  edge of an earlier, already-finished one) or connected afterward by a
  net/mesh of chains and slip stitches worked between finished motifs.
- **Amigurumi**: separate parts (arms, legs, ears, tail) are commonly
  worked as their own independent pieces from their own starting
  points, fastened off individually, then **sewn** onto the body with a
  separate length of yarn (often the piece's own tail) rather than ever
  being on the same working thread as the body.

These are two genuinely different join mechanisms, worth keeping distinct
when this is actually designed:

1. **Crochet joins** — worked live with the hook: a new stitch's loop is
   drawn through a point on an already-finished thread instead of (or in
   addition to) the current one. Structurally this is just an ordinary
   insertion-target edge (§4/§5) whose target happens to belong to a
   different thread than the stitch being worked — no new primitive
   needed, just permission for a target to cross threads.
2. **Sewn seams** — worked *after* fastening off, with a separate, usually
   short, length of yarn whip-stitching two finished edges together at
   discrete points. This is not a shared-loop insertion in the §3 sense at
   all (no hook, no draw-through recipe) — it's a weaker "these two points
   are now attached" edge, closer to a fixed-distance constraint for the
   §6 relaxation solve than to a stitch.

**Model implication for M1:** the top-level "scheme" object should be a
**list of one-or-more threads** from the start (each thread internally a
working-order sequence per §4), even though the earliest milestones will
only ever populate that list with exactly one thread. Treating "thread" as
an implicit singleton at the top level now would make multi-thread support
later a restructuring; treating it as a list from day one makes it
additive. The two join-edge kinds above don't need to exist yet — just
don't build a data model that assumes there is only ever one thread in a
scheme.

## 5. Shaping — increases and decreases

- **Increase**: two or more stitch instances share the same insertion
  target. Geometrically, one earlier point in the piece now has two (or
  more) stitches' worth of thread anchored to it — the fabric widens
  locally. This is the mechanism behind flat circles (a fixed increase
  count per round, e.g. 6 per round for a standard flat circle in `dc`)
  and all outward shaping, including the *non*-constant increase rates
  that produce hyperbolic/ruffled surfaces.
- **Decrease**: a single stitch instance has multiple insertion targets —
  it begins normally but, instead of completing over one target, pulls its
  final draw-through through loops raised from **two or more** separate
  earlier points (e.g. `dc2tog`: pull up a loop from each of two targets —
  3 loops on hook — then one draw-through clears all three). Geometrically,
  two earlier points collapse into one stitch above them — the fabric
  narrows locally.
- Both are just **the general "how many insertion targets does this stitch
  have, and how many stitch instances share a target" property** from §4 —
  not new primitives. 1-target/not-shared is the default; increases and
  decreases are simply other values of the same property.

## 5a. Multi-way shares and target capacity

**Decision (2026-08-24/25, Owner, prompted by a question about lace).**
Real insertion points have a **physical capacity** for how many stitches
can share them, and that capacity — and what happens once it's exceeded —
depends on *what kind of point* it is, not just "how many stitches." The
Owner's own calibration, which the engine's constants are tuned against:

- **An ordinary stitch's top** (the usual increase target): comfortably
  fits a handful of siblings; **seven is hard but possible**; **eleven
  won't fit** — real yarn thickness makes it physically impossible, not
  just visually crowded.
- **A magic ring, tightened** (the ordinary way one is used): **3-5
  stitches** cinch into a small radius and read as a **narrower, pointier
  3D shape** rather than a flat disc; **6-8** is the flat-circle sweet
  spot (the classic amigurumi round-1 start); **9+** can't open the ring
  any further and **ripples into a wavy 3D circle** instead; far beyond
  that, it's not that the shape gets stranger — it's that a ring genuinely
  **cannot be tightened around that many strands at all**, a hard physical
  limit, not a gradient.
- **A magic ring left deliberately open** (not tightened): behaves
  differently from the tightened case above — no cinch point forcing a
  small radius, so it can simply widen to fit more, the same way the next
  case can.
- **A chain or chain-space** (the gap enclosed by a run of chain stitches,
  e.g. "3dc in the ch-3 space" in a granny square) — a genuinely different
  construction from a magic ring (a chain is a run of pulled-through
  loops; a magic ring is one loop of working yarn, §4's foundations note)
  that happens to land in the same "very elastic" bucket for capacity
  purposes: comfortably widens to fit more without the same physical
  ceiling as an ordinary stitch or a tightened ring. Granny squares
  typically use 3; lace and freeform work can call for very different
  counts, and the
  engine needs to handle that range, not just the granny-square norm.

**Model implication.** Every stitch, as a *target* for others, has a
`CapacityStyle` (§3a's registry, so it's per-kind and overridable per
instance for cases like "this specific ring was left open"):
- `Fixed` — ordinary stitches (`dc`, `tr`, ... `quad_tr`, `ss`). Small,
  roughly constant comfortable capacity; past it, extra siblings don't
  crowd further in-plane, they bulge out of the flat plane (a 3D ripple)
  — and that bulge is deliberately bounded, so a truly excessive count
  still ends up geometrically too close for §8 invariant 4's checker to
  wave through, which is what makes "eleven won't fit" a real, discoverable
  fact rather than something the placement code has to hard-code as a
  rule.
- `TightenedRing` — a magic ring's default. Radius **grows with sibling
  count up to a flat plateau** (few siblings stay narrow/pointy, more
  reach the flat-circle plateau), then behaves like `Fixed` beyond it
  (ripples, same bounded-bulge reasoning).
- `Elastic` — `ch` (and a magic ring explicitly left open, via the
  per-instance override). Radius keeps growing with sibling count, no
  plateau, no rippling — "chain geometry is very elastic," per the Owner.

One shared "comfortable capacity" threshold (currently 7) serves both the
ordinary-stitch case and the tightened-ring case reasonably, since the
Owner's own numbers for both (7 hard-but-possible; 6-8 the ring's flat
range) sit close together — no separate tuning needed per target kind.

**Geometric mechanism.** Siblings sharing a target are placed **radially**
around it, not linearly along one axis — a linear offset has no way to
represent "opens into a wide circle" or "ripples in 3D," and (see
`HANDOVER.md`) turned out to let non-adjacent siblings fold into each
other during relaxation, since nothing but the linear ordering related
them at all. The relaxation solver (§6) also gained an explicit
**repulsion** between every pair of stitches sharing a target, active
only once they're closer than a minimum distance — springs alone only
relate a stitch to its own target and its immediate working-order
neighbour, never to siblings further round a wide fan, so nothing stopped
a wide share from folding onto itself before this was added.

The angle itself is **not** simply `2*PI*index/total` for every target
kind (found while building M4's demo scheme, a magic-ring flat circle):
that wraps every group around a full circle regardless of size, which is
right for a target that genuinely represents "the whole round" (`Elastic`
and `TightenedRing` — nothing else is nearby to collide with) but wrong
for an ordinary `Fixed` increase into one existing stitch in the middle of
an otherwise dense round, where it let one increase's far sibling swing
into a *neighbouring* increase's territory. So: `Fixed` targets fan out
gradually (a fixed angular step per sibling) for as long as that stays
under a full turn, only wrapping the full circle once the group is large
enough that it would anyway (matching `COMFORTABLE_CAPACITY`'s own
regime); `Elastic`/`TightenedRing` targets always wrap the full circle,
at any size, since they represent an isolated round, not an increase
embedded in one.

**Known, deferred limitation — local density across *different* targets.**
This model still doesn't reason about how close two *different* targets
are to each other: a dense round's several increases, each individually
correctly placed against *its own* target, can still collide with a
*neighbouring* increase's children if the round is tight enough (found
building M4's demo — a magic ring's round 1 plus a plain round-2 increase
overlapped, so the shipped M4 demo deliberately stops at round 1). Fixing
this properly needs the placement/relaxation to be aware of nearby
targets' occupancy, not just a single target's own sibling count —
real, scoped follow-up work, not a tuning fix (constant-tuning was tried
and made other cases worse). Not fixed as of M4; see `HANDOVER.md`.

**Known simplification, stated plainly:** the "pointy" look for a
lightly-loaded tightened ring is modelled only as a *narrower radius*
(shorter distance from the ring to where the siblings sit), not as an
actual per-stitch inward/outward *lean*. That's a reasonable proxy for
"reads as more vertical/pointed than a flat disc," not a claim about the
precise silhouette real yarn would form — flagged here rather than
overclaiming precision the Owner's description didn't fully pin down.

## 5b. Front/back loop and post as genuinely separate points

**Decision (2026-08-25, Owner).** A stitch's top loop (§2's FLO/BLO) has
two strands — the **front** strand (the Owner's own description: the half
facing toward the right, relative to the direction of crocheting) and the
**back** strand (facing left, relative to that same direction). Working
into only one strand — as most stitches don't, but some deliberately do —
**leaves the other strand free for a different, later stitch to use.**
That has real visual and structural consequences: it isn't a preference
about which half to "count," it produces two genuinely distinct available
insertion points on the same stitch.

**Model implication.** `LoopTarget::FrontOnly`/`BackOnly` (already in the
graph model, §4 — `StitchInstance.loop_target`) previously had **no
geometric effect at all**: a stitch marked `FrontOnly` and one marked
`BackOnly`, both targeting the same earlier stitch, would place
identically apart from ordinary sibling-ring spacing (§5a) — nothing
represented that they'd actually landed on two different strands. Fixed:
`geometry.rs` now applies a real offset (`LOOP_HALF_OFFSET`) for
`FrontOnly`/`BackOnly`, on the same axis `FrontPost`/`BackPost` (§2's post
stitches — attaching to the *leg*/post of an earlier `tr`/`dtr` rather
than its top loops, for raised/cable texture) already used, just a
smaller displacement — picking one strand of a loop is a smaller physical
move than reaching around an entire post, though empirically not
*dramatically* smaller (see `geometry.rs`'s comment on the exact value
picked, and why).

**Confirms an existing capability, not a new one:** targeting a stitch
several rows back (not just "the row below") was already fully supported
by the insertion-graph model (§4 — there's no special-casing for how far
back a target is). **Mosaic crochet** is the concrete case that ties
FLO/BLO and long-range targeting together: a row worked entirely into
back loops only, leaving every front loop in that row free, then a later
row (skipping the back-loop row entirely) reaches back with a taller
stitch into those left-free front loops. Added as an explicit test case
(`validate.rs`) — both because it's a good end-to-end check of §4 + §5b
working together, and because it's exactly the kind of scheme this
project exists to let a designer verify without a physical trial.

**Not addressed by this round:** nothing in the engine yet *prevents* two
different stitches from both claiming the same strand of the same target
(e.g. two stitches both marked `FrontOnly` on the same stitch) — that
would be a real pattern error, not just a geometry question, and would
need its own consistency check (an extension of `validate.rs`, or a new
checker) rather than falling out of the geometry/self-intersection work
here. Flagged, not built.

## 5c. Highlighting/inspection granularity (viewer, deferred)

**Decision (2026-08-25, Owner)** — for M4/M5 (no viewer exists yet, so
nothing here is built; captured now so the design intent isn't lost):

- **Default view**: a stitch highlights as **its loop** (the top "V"), or
  **its leg/post** when it has one distinct from the loop (`tr` and
  taller — the same "post" `FrontPost`/`BackPost` already attach to, §2).
  These are the two coarse, always-available regions of any tall stitch.
- **Detailed view** (opt-in): every point where the yarn, once folded/
  relaxed, **touches itself** becomes its own separately highlightable
  part. This maps directly onto engine machinery that already exists for
  a different purpose: `crate::path`'s raw-placement point coincidence
  (`crate::validate`'s adjacency rule, §8 invariant 4) already identifies
  exactly this — "these two points in the yarn's path are the same
  structural point" — for deciding what's *not* a self-intersection. The
  detailed highlighting view is essentially a UI presentation of that
  same underlying structure, not a new geometric concept to invent.
- **"Every hole between threads can be a target."** Two readings worth
  keeping distinct when the editor (M5) is actually designed:
  - A hole **marked by an explicit chain** (a "ch-1 space," "ch-3
    corner space," etc.) — already fully supported: `ch` is `Elastic`
    (§5a), so it comfortably accepts however many stitches a scheme calls
    for, from a granny square's 3 up to whatever lace needs. No new work.
  - A hole **not marked by any stitch at all** — e.g. a skipped-stitch
    gap in filet mesh, or a gap that emerges purely from a decrease's
    geometry — has no `StitchRef` for anything to target, because nothing
    was placed there. Supporting this as a real target (not just "point
    the editor at empty space and let the user pretend") would mean a new
    kind of target reference — a *derived/virtual* anchor computed from
    the stitches that bound the gap, distinct from `StitchRef` — which is
    a genuine architectural addition, not a tuning fix. Not attempted
    here; flagged for real design work whenever the editor's interaction
    model is actually being built, rather than guessed at now.

## 6. Elasticity — a topology property, not a yarn property

**Decision (2026-08-24, Owner):** the fabric's elasticity/stretchiness
must be simulated, but **elasticity is a property of stitch topology (how
loops connect to and can pivot relative to their neighbours), not a
property of the yarn material itself.** In other words: don't model
elasticity by giving the yarn a spring constant and stretching it like an
elastic cord — model it by letting the insertion-graph connections (§4)
have some range of relative motion, the way real interlocked loops do,
and let the fabric's stretch behaviour emerge from solving that graph
under load/relaxation.

Implications for the engine (to work out in detail when this milestone is
reached, not now):

- Each insertion-target edge (§4/§5) is a **constraint with some give**
  (a loop can pivot and shift somewhat relative to what it's drawn
  through), not a rigid fixed offset — this is what makes dense stitch
  patterns (e.g. `dc`) comparatively inelastic and open/tall stitch
  patterns (e.g. `tr`, mesh, chain spaces) comparatively stretchy, purely
  as a consequence of how much freedom the graph's connections allow, with
  no separate "material stretchiness" number involved.
- A **relaxation/solve step** (settling the graph to an equilibrium shape,
  and re-solving it under an applied stretch) is therefore a real part of
  the simulation, not just static placement — this sits between raw stitch
  placement (§3/§4) and the self-intersection check (§8 #4), since you
  want to validate the *relaxed* shape, not the naively-placed one.
- Gauge (§2) stays a separate, currently-out-of-scope concept: gauge is
  about absolute size for a given tension/hook/yarn combination; elasticity
  is about how much a given topology can deform from its rest shape. The
  engine can model the latter without ever modelling the former.

This directly affects the milestone plan — see the note added to
`GOALS.md` M1/M2.

## 6a. 2D vs 3D construction space (deferred)

**Decision (2026-08-24, Owner):** the app will eventually need both a
**2D mode**, for pieces designed to lie flat (doilies, granny squares,
lace motifs), and a **3D mode**, for volumetric pieces (amigurumi, bowls,
bags). **Confirmed out of scope for now — must be addable later without a
redesign.**

Working note on how this likely fits the model above, to revisit in detail
when it's actually scheduled (not a final decision):

- The relaxation physics in §6 is inherently general — flat vs. curved
  equilibrium shapes already fall out of topology (insertion-graph density
  and increase/decrease schedule, per §8 invariant 5) without needing two
  separate physics models. A flat circle stays flat because its increase
  rate matches its growing circumference; a bowl curves because its
  increase rate falls behind partway through, exactly the same solver
  either way.
- Given that, "2D mode" is probably best built as **(a)** a flat-pattern
  editing/viewing convenience (a 2D charting-style view, useful before
  ever running the 3D solver) plus optionally **(b)** a solver constraint
  that keeps a piece's relaxed shape planar on purpose (e.g. for precise
  lace charting, or just to avoid tiny numerical out-of-plane wobble on a
  piece that's supposed to lie flat) — rather than a second, separate
  simulation engine. This also naturally covers pieces that are flat at
  the start and become 3D partway through (a bowl's flat base transitioning
  to its walls) without needing a hard mode-switch mid-piece.
- This is a design fork worth a real decision when M4/M5 (viewer/editor)
  or a later milestone actually reaches it — flagged here so the
  relaxation solver (§6) and viewport (M4/M5 in `GOALS.md`) aren't built
  in a way that assumes "always full unconstrained 3D" and blocks adding
  the constraint later.

## 7. Pattern notation conventions (for later pattern-import work)

Not needed for the earliest milestones, but worth having on record since
it'll matter once schemes can be typed/pasted in as text:

- `*` … `*` or `[` … `]`, then "repeat from * N times" / "repeat 3 times":
  marks a repeating group.
- `(dc, htr, dc) in next st`: work all three stitches into one insertion
  point (an increase, or a shell/fan).
- Numbers in parentheses at the end of a row/round, e.g. `(18 sts)`: the
  expected total stitch count after that row/round — a built-in self-check
  the engine's validator should be able to reproduce and flag mismatches
  against, computed from the insertion graph (§4) rather than from any
  stored "row" object.

## 8. Geometric/topological invariants for the simulator

Distilled for the engine, not general crochet teaching. Written against
the insertion-graph model in §4 — nothing here assumes rows are real
objects.

1. **Per thread** (§4a — a scheme may eventually contain more than one),
   the yarn is a single continuous curve with exactly one free end (the
   working loop) at any point during that thread's construction; a
   finished, fastened-off thread has two free ends (start tail + finish
   tail) and is otherwise a closed/continuous path. This is a per-thread
   invariant, not a whole-scheme one — a multi-thread scheme is several
   independently-continuous curves plus join edges between them (§4a),
   not one giant continuous strand.
2. Every stitch has exactly one insertion target **except**: `ch`, which
   always has **zero** (formed purely from the working loop — a fixed
   fact of the stitch, not a per-instance choice, see §3); increases
   (target shared with sibling stitches); and decreases (multiple targets
   collapsed into one stitch) — see §5. There is no separate exception for
   spike stitches, turning chains, or freeform work; "target is further
   back, or anywhere, in working order" is just an ordinary value of the
   same property, not a special case (this is the direct consequence of
   dropping rows as real objects — see §4).
3. The total stitch count reachable from any point in the graph is fully
   determined by walking it and summing each stitch's target-sharing/
   target-count behaviour — this is what reproduces the `(N sts)`
   self-check in §7 for conventional row/round work, and generalises
   cleanly to freeform/hyperbolic work where there's no fixed "row total"
   to check against, only local consistency at each insertion point.
4. Real yarn cannot pass through itself. Ordinary stitches never require
   it to; **post stitches and any deliberately overlapping/cable-like
   technique are the only constructions where the thread's rendered path
   legitimately crosses itself in projection without being a physical
   contradiction** — the self-intersection checker needs to treat true 3D
   coincidence (after the §6 relaxation step, not before) as an error, but
   treat "crosses in projection because a post stitch reaches past its
   neighbours" as expected, not a false positive. ⚠ Getting this
   distinction right is probably the trickiest part of this milestone —
   flag for extra test-case coverage.
5. Stitch height (§3) plus local insertion-graph density together
   determine the fabric's local curvature *and* its elasticity (§6) — this
   is *why* flat circles need a fixed increase rate, tubes need zero net
   increase/decrease per round, hyperbolic surfaces need a faster/varying
   increase rate, and shaped pieces (amigurumi bodies, etc.) are just a
   deliberate schedule of increases/decreases/plain stretches. Worth
   keeping as a sanity-check axis for "does this scheme look physically
   plausible," not just raw self-intersection.

## Open questions for the Owner

- Multi-language stitch-name recognition (§1): no timeline requested yet —
  fine to leave as an architectural constraint (canonical IDs + mapping
  layers) until a milestone actually needs it. Flag if a specific language
  should be prioritised first (e.g. for a specific pattern source the
  Owner wants to import from).
- Elasticity (§6) changes the shape of the milestone plan (a relaxation/
  solve step becomes core, not optional) — see the proposed re-plan noted
  in `GOALS.md`. Needs Owner sign-off before M1 starts.
- 2D/3D construction-space modes (§6a): confirmed deferred by the Owner.
  Revisit the "one solver, optional planar constraint" vs. "two separate
  models" fork (§6a) for real once M2 (relaxation) or M4/M5
  (viewer/editor) is actually being built — not needed before then.
- Multi-thread schemes and joins (§4a): confirmed deferred by the Owner.
  The crochet-join vs. sewn-seam distinction (§4a) needs a real decision
  (e.g. does the relaxation solver treat a sewn seam as a rigid or
  slightly-elastic constraint?) once this is actually scheduled — not
  needed before then, but M1's scheme object should already be a list of
  threads, not an implicit singleton.
- Multi-way share fidelity (§5a): the "pointy tightened ring" look is
  currently a narrower radius only, not a true per-stitch lean — flagged
  as a known simplification, not verified against real crochet's actual
  silhouette. Revisit if a future milestone (viewer/editor) needs the
  visual shape to be more than "reads as roughly right."
- Loop/strand consistency checking (§5b): nothing currently stops two
  different stitches from both claiming the same strand (e.g. both
  `FrontOnly`) of the same target, which would be a real pattern error.
  No timeline requested — noted so it doesn't get assumed already covered
  by the self-intersection checker (it isn't; that's a geometric check,
  this would be a graph-consistency one).
- Local density across different targets (§5a): a dense round's several
  increases can collide with a *neighbouring* increase, since placement
  only reasons about siblings of the same target. M4's demo deliberately
  stops at round 1 of a flat circle to avoid shipping a scheme that hits
  this. No timeline requested — worth prioritising before M5's editor
  lets an Owner build a real multi-round piece and hit it directly.
