# Crochet Sim — Handover

## Current state

**M1 done (2026-08-24).** `core/` is a Rust crate implementing:
- `stitch.rs` — the open `StitchRegistry` (§3a), seeded with the basic UK
  ladder (`ch`, `ss`, `dc`, `htr`, `tr`, `dtr`, `trtr`, `quad_tr`) as
  parametrised pre-wrap/draw-through recipes (§3), including the
  htr-vs-tr height distinction (D10 area) and `ch`'s zero-target,
  zero-insertion special status.
- `graph.rs` — the insertion graph (D4): `Scheme` is a `Vec<Thread>` (D9,
  never a singleton even though only one thread is ever populated so
  far), `Thread` is a working-order `Vec<StitchInstance>`, each instance
  carries `targets: Vec<StitchRef>` (0 for `ch`, 1 normal, 2+ decrease,
  shared across instances for an increase) — `StitchRef` already supports
  cross-thread indices for the future D9 join case, unused for now.
- `geometry.rs` — raw (unrelaxed) 3D placement: walks each thread in
  working order, positions each stitch from its target(s)' already-placed
  position plus its height, and returns an explicit `Result` (never
  panics) if a target isn't placed yet. Deliberately naive — no
  elasticity (M2) or self-intersection checking (M3) yet.

17 unit tests cover: `ch`/`ss` zero-target/zero-height special cases,
strictly-increasing stitch heights including the htr/tr distinction,
registry extensibility, a conventional row-into-a-chain scheme, an
increase spreading its siblings, a decrease averaging its targets, a
spike stitch (target further back than the immediate predecessor), a
fully non-row freeform scheme, and a not-yet-placed-target error path.
`cargo clippy --all-targets` is clean; `cargo fmt --all` applied.

**M2 done (2026-08-24).** `core/src/relax.rs`: a mass-spring relaxation
solver over the same insertion graph. Every insertion-target edge and
every working-order continuity edge (consecutive stitches in a thread —
new in M2, not present in M1's placement) becomes a Hookean spring
(`force = stiffness * (distance - rest_length) * direction`), integrated
with damped semi-implicit Euler to a settled equilibrium. Stiffness comes
from a new `StitchDef::insertion_stiffness()` (§6: dc stiff/low-give,
htr/tr/dtr/... progressively softer, `ch` loosest) — elasticity is
entirely a consequence of stitch kind, no separate material parameter
anywhere in the solver. `RelaxationParams.pinned` lets a caller hold
specific stitches at fixed (possibly displaced) positions — this is both
"hold an edge" and "apply a stretch," the same mechanism.

Verified: a swatch already at its M1 rest state barely moves under
relaxation (sanity/idempotency); pinned stitches stay exactly put; and —
the actual M2 deliverable — pulling the last stitch of a `dc` row sideways
drags its free neighbour noticeably less (1.42 units, under a pull of
length 3) than the same pull applied to a `tr` row (1.70) or a `dtr` row
(1.76), confirmed by inspecting real numbers, not just an assertion
passing. 22 unit tests total (was 17 after M1), clean under
`cargo clippy --all-targets`, `cargo fmt` applied.

**M3 done (2026-08-24).** Two new modules:
- `core/src/path.rs` — reconstructs each thread's *complete* relaxed yarn
  path: every stitch's own base-to-top sub-path, plus a "bridge" segment
  connecting consecutive stitches in working order whenever their
  positions don't already coincide (the physical strand between them —
  new in M3; M1/M2 never modelled this gap explicitly). Also: added
  `POST_DEPTH_OFFSET` to `geometry.rs`'s placement, so a front/back post
  stitch's base carries a real depth offset — this is what makes a post
  stitch's path genuinely not occupy the same 3D space as the stitch(es)
  it reaches past, rather than requiring the checker to special-case it.
- `core/src/validate.rs` — `check_self_intersections`: flags any two
  non-adjacent segments closer than a yarn-diameter threshold
  (`DEFAULT_YARN_DIAMETER = 0.15`, smaller than `POST_DEPTH_OFFSET = 0.4`
  on purpose). Also `check_round`: the programmatic `(N sts)` self-check
  (docs §7/§8 invariant 3), generalised for a model with no real row
  objects — the caller names the previous/new round's stitch refs
  explicitly and it checks count + that every new stitch's target(s)
  actually lie in the claimed previous round.

**The adjacency problem was harder than expected and worth recording.**
First attempt: exclude segment pairs sharing a literal `StitchRef`. Wrong
— missed that plain positionally-consecutive segments (e.g. two chain
links) touch without sharing a ref. Second attempt: also link a
zero-target stitch with its working-order predecessor. Wrong the other
way — this also (correctly only for zero-target stitches, but the bug was
applying it more broadly during iteration) risked hiding genuine
collisions between stitches that merely happen to be consecutive but
place their geometry from *different* targets. Landed on: each stitch has
a **1-hop neighbourhood** (itself, its target(s), and — only when it has
no targets — its immediate predecessor), and two segments are adjacent
iff their owning stitches' neighbourhoods *overlap* (not identical, not
full transitive closure — deliberately just one hop, since a naive full
transitive closure over the graph eventually connects everything through
the working-order backbone and would hide real collisions). Verified this
distinction with a deliberately-engineered bad case: two *unrelated*
stitches (different targets, no shared neighbourhood) pinned to the exact
same relaxed position — correctly flagged, while an ordinary swatch and a
front-post-stitch case (the milestone's required non-false-positive test)
both correctly pass. Also had to fix an unrealistic test-scheme detail
along the way: rows must target the row below in *reverse* order (real
crochet turns at the end of each row) — same-direction targeting created
a pathological bridge running the full width of the previous row.

30 unit tests total (was 22), clean under `cargo clippy --all-targets`,
`cargo fmt` applied. Known, documented limitation (not an oversight): the
1-hop neighbourhood rule is deliberately permissive — it will not catch
every conceivable true self-intersection between structurally-adjacent
stitches, favouring zero false positives (the milestone's explicit
priority) over exhaustive true-positive detection.

**M3 correction (2026-08-24, prompted by an Owner question about lace).**
Owner asked whether lace — where many stitches routinely share one
insertion point (shells, motifs) — validates correctly. It didn't, fully.
Investigation found two distinct problems, one fixed, one still open:

1. **Fixed — adjacency was graph-topological, not geometric.** The
   original M3 adjacency rule (a stitch-reference "neighbourhood overlap")
   excluded *any* two stitches sharing a target from checking against each
   other at all, not just against the shared target. Verified this let
   two shell siblings pinned to ~0.01 apart pass silently — exactly the
   lace case. **Replaced with raw-placement point-coincidence**: two
   segments are adjacent iff an endpoint coincides in the *raw* (M1,
   pre-relaxation, pre-pinning) placement — a purely geometric fact fixed
   at raw-placement time, not something relaxation or a test's pinning
   can accidentally satisfy or fake. `path.rs`'s `PathSegment` now carries
   `raw_start`/`raw_end` alongside the relaxed pair for this purpose.
   `validate.rs`'s module doc has the fuller account of why the old rule
   was wrong and why this one isn't. Regression tests added: an ordinary
   3-tr shell passes, the same shell with two siblings pinned together is
   caught.
2. **Fixed — `INCREASE_SPREAD_X` was too small for anything taller than
   `dc`.** Investigating (1) surfaced a second, more basic problem:
   *any* stitch with more than one own-path sub-segment (`tr` and up),
   immediately followed by a sibling sharing its target — an entirely
   ordinary 2-stitch increase, not lace-specific at all — put that
   sibling's lower sub-segment right at the edge of
   `DEFAULT_YARN_DIAMETER` from the connecting bridge (~0.148 vs. a 0.15
   threshold, essentially by construction: the near-miss distance is
   roughly `INCREASE_SPREAD_X / 2`). Raised `INCREASE_SPREAD_X` from 0.3
   to 0.5 to clear it with real margin; comment in `geometry.rs` records
   the reasoning and the arithmetic so it isn't silently dropped back to
   0.3 later.
3. **Still open — wide multi-way shares can fold during relaxation.**
   Testing shells up to size 7 surfaced a *deeper* issue, present even
   before today's changes and unrelated to `INCREASE_SPREAD_X`: with
   ~5+ stitches sharing one target, M2's relaxation solver has no
   bending/repulsion resistance keeping the fan of siblings from curling
   onto itself — confirmed non-adjacent siblings (e.g. index 6 vs index 7
   in a 7-`dc` shell) can end up essentially coincident (~1e-16 apart)
   after 150 relaxation steps. M3 correctly flags the resulting overlap
   (it's a real one), but a design tool that can't validate a 5+ stitch
   shell without a false alarm is a real gap for lace specifically, where
   wide shells are common. **Not fixed in this session** — would mean
   adding an angular/bending term or an explicit same-target repulsion to
   M2's spring model, which is a real (if bounded) design task, not a
   tweak. Flagged to the Owner; revisit as its own piece of work if lace
   support is a near-term priority, otherwise it's fair to leave as a
   known limitation for now (documented here and in `validate.rs`'s
   module doc so it isn't rediscovered from scratch).

32 unit tests total (was 30 immediately after M3). Clean under
`cargo clippy --all-targets`, `cargo fmt` applied.

Not started: M4 (WASM bridge + minimal viewer) onward. Goal G-001's
6-milestone plan is in `GOALS.md`, approved by the Owner 2026-08-24.

## Decision record

**D1 — Standalone web app, not a Blender plugin (2026-08-24).**
Owner chose "standalone app" over a Blender plugin when asked, because it
gives full control over a UI purpose-built for scheme design rather than
being constrained by Blender's panel system and Python API/versioning, and
doesn't require Blender to be installed to use the tool. A Blender plugin
remains a plausible later add-on (same core, thin wrapper) if the Owner
wants to reuse Blender's viewport/renderer down the line — not ruled out,
just not the initial target.

**D2 — Rust core, compiled to WASM, behind a Next.js/TypeScript UI
(2026-08-24).** The task requires a compiled language for the performance-
critical part (yarn-path geometry and self-intersection checks over
potentially thousands of stitches). Options considered:
- *Rust → WASM + Next.js/TS frontend* (chosen): matches the portfolio's
  existing stack for everything except the core (`listing-studio` and
  `when-we-meet` are both Next.js/TypeScript/Prisma/Postgres web apps —
  see `E:\CLAUDE\COMPANY\STANDARDS.md` "minimize spread"), reuses the same
  Vitest+Playwright test setup and the same Docker/nginx deploy pattern
  documented in `E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`, and Rust-to-WASM is
  a well-trodden path for browser-side geometry/physics work. Rust's
  ownership model also suits a geometry kernel with lots of shared curve
  data and no GC pauses during simulation.
  - No pure-persistence "database" is needed for the simulation itself —
    schemes are documents, not relational data — but the app will still
    likely want Postgres for saved-scheme storage/accounts later, which
    also matches the portfolio.
- *C++ core, native desktop app*: stronger geometry/physics library
  ecosystem and is Blender's own implementation language (would help *if*
  a Blender plugin is built later), but a from-scratch native desktop app
  breaks from the portfolio's web-app pattern for no strong reason given
  the Owner picked "standalone" over "Blender plugin" — rejected for now.
  Revisit if profiling ever shows WASM is a real bottleneck, or if the
  Blender-plugin option is picked back up later.
- *C# / Unity*: good 3D tooling out of the box, but introduces a whole new
  ecosystem to the portfolio with no existing project to share it with —
  rejected per "minimize spread."

**D3 — MVP targets full 3D yarn simulation, not a 2D chart-only first cut
(2026-08-24).** Owner's explicit choice when asked. This makes M1/M2 (see
GOALS.md) bigger than a 2D-first plan would have been, but matches the
goal's own framing ("simulating a yarn thread, which will be folded and
intersected multiple times") more directly than a flat-chart intermediate
step would.

## How things fit together

Not yet built. Planned shape (subject to revision as M1 proceeds):
- `core/` — Rust crate: an **insertion graph** of stitch instances (working
  order + insertion-target edges — see `docs/crochet-context.md` §4,
  `graph.rs`), an extensible stitch registry (§3a, `stitch.rs`), raw
  placement geometry (`geometry.rs`, M1), a mass-spring relaxation/
  elasticity solve (§6, `relax.rs`, M2), continuous relaxed-path
  reconstruction (`path.rs`, M3), and self-intersection/count validation
  on that path (§8, `validate.rs`, M3 — all done). Pure Rust,
  unit-testable without any UI, compiled to WASM (`wasm-bindgen`) for the
  browser eventually (M4).
- `web/` — Next.js/TypeScript app: scheme editor UI + a 3D viewport
  (likely three.js / react-three-fiber) that calls into the WASM core and
  renders its output, highlighting any flagged geometry problems. The
  editor must not assume row/round structure — see D4 below.

## Next steps

M4 (WASM bridge + minimal viewer): compile `core` to WASM
(`wasm-bindgen`), wire it into a minimal Next.js/TS app with a 3D
viewport (three.js / react-three-fiber) rendering a hardcoded sample
scheme end-to-end in the browser — the *relaxed* (M2) shape, with a
visible flag when M3's validator detects a problem. This is the first
milestone that touches `web/` / TypeScript at all; nothing exists there
yet. Per `docs/crochet-context.md` §6a, don't build the viewport in a way
that assumes unconstrained-3D-only.

## Domain reference

`docs/crochet-context.md` — UK/GB crochet terminology and construction
rules, written specifically to inform the engine's data model. Marked with
⚠ wherever a detail needs real crochet-literate review before being encoded
as a hard rule — check those before M1 locks in the stitch primitives.

## Decision record (continued)

**D4 — Core model is an insertion graph, not row/round objects
(2026-08-24, Owner correction).** The first draft of `docs/crochet-
context.md` modelled rows/rounds as structural units the engine reasons
about (turning chains between rows, "insert into the previous row," etc.).
Owner corrected this: freehand/freeform crochet, hyperbolic crochet, and
other exotic styles don't work in rows at all, so a row-based core model
would need special-casing or a rewrite to support them. The actual model:
a single continuous thread as an ordered sequence of stitch instances in
working order, each with one or more insertion-target references to
earlier stitch instance(s) (see `docs/crochet-context.md` §4). Rows/rounds
become a derived, optional grouping — useful for pattern-text generation
and UI, never load-bearing in the simulation core. Increases/decreases,
spike stitches, post stitches, and freeform placement all fall out of the
same "how many insertion targets, how many stitches share a target"
property, with no separate exception-handling per technique.

**D5 — Elasticity is simulated as a topology property, not a yarn
property (2026-08-24, Owner).** The fabric must visibly stretch/deform
realistically, but that behaviour should emerge from how much relative
motion the insertion-graph's connections allow (dense stitches = less
give, open/tall stitches = more give), not from giving the yarn itself a
spring constant. This means a **relaxation/solve step** is a real part of
the simulation pipeline — settling the graph to equilibrium and re-solving
it under stretch — sitting between raw stitch placement and the
self-intersection check (validate the *relaxed* shape). See
`docs/crochet-context.md` §6 for detail; the milestone plan below folds
this in as its own milestone rather than bolting it onto geometry
validation.

**D6 — Multi-language stitch-name recognition is a future capability, UK
stays canonical now (2026-08-24, Owner).** Beyond the US↔UK distinction
already logged, stitch-name recognition should eventually cover other
languages/naming systems too. No timeline yet, but it means the stitch
registry (D4, `docs/crochet-context.md` §3a) must key stitches by a stable
internal ID with localized-name mapping layers on top, from the start —
not a UK-abbreviation-as-key data model that a later multi-language effort
would have to unwind.

**D7 — Textured/compound stitches and other stitch traditions are
deferred but must not require a redesign to add (2026-08-24, Owner).**
Clusters/shells/bobbles/popcorns/etc. stay out of scope through the early
milestones, confirmed by Owner, on the condition that the stitch registry
(D4, §3a) is built open/extensible (composition of simpler
stitches/insertions) rather than a closed enum, so adding them later is a
registration, not a rewrite.

**D8 — 2D vs 3D construction-space modes are deferred but must stay
addable (2026-08-24, Owner).** Flat pieces (doilies, granny squares, lace)
and volumetric pieces (amigurumi, bowls, bags) will eventually need
distinct modes, but this is explicitly out of scope for now. Working
assumption (not final, see `docs/crochet-context.md` §6a): the D5
relaxation solver is already general enough that flat vs. curved shape
emerges from topology alone, so "2D mode" is likely a flat-pattern
viewport/editing convenience plus an optional planar constraint on the
solver, not a second physics engine — revisit for real when M2 (relaxation)
or M4/M5 (viewer/editor) is actually being built. Noted here so those
milestones aren't implemented in a way that assumes unconstrained 3D only.

**D9 — Schemes must support multiple threads/joins later; the scheme
object is a list of threads from the start (2026-08-24, Owner).** Real
crochet often builds a piece from more than one separately-started thread:
Irish crochet motifs worked independently and then joined (either live,
by crocheting through an already-finished edge, or afterward via a
chain/slip-stitch net), and amigurumi parts (limbs, ears) worked
separately and then sewn onto the body with their own tail. Confirmed out
of scope for now, but the top-level scheme object must be a **list of
one-or-more threads** from M1 onward (each thread internally the D4
insertion graph), even while only ever containing one thread in the
earliest milestones — treating it as an implicit singleton now would make
multi-thread support later a restructuring rather than additive. The two
join mechanisms (a same-thread-model "crochet join" edge vs. a
structurally different "sewn seam" constraint) don't need designing yet —
see `docs/crochet-context.md` §4a.

**D10 — Chains have zero insertion targets; turning chains are not a
special case (2026-08-24, Owner correction).** `ch` never has an insertion
target at all — it's formed purely from the working loop, unlike every
other stitch (which has exactly one, or several for increases/decreases).
This was already implicit in the terminology (§1: "pull a new loop through
the loop on the hook") but D4's write-up hadn't stated it as a formal
model fact, and had wrongly implied turning chains carry some special
per-convention ambiguity (⚠ flag, now removed). Corrected: a turning chain
is structurally identical to any other chain (zero targets); the only
thing that varies by convention is an ordinary property of the *next*
stitch after the turn (which earlier point it targets), already covered by
the general insertion-target mechanism — no chain-specific rule needed
anywhere in the engine. See `docs/crochet-context.md` §3/§4/§8 invariant 2.

## Milestone re-plan pending (2026-08-24)

D4/D5/D6/D7 above change the shape of the milestone plan from what was
originally proposed (5 milestones, row-based M1, self-intersection-only
M2). See `GOALS.md` for the proposed 6-milestone replacement — needs
Owner sign-off before M1 starts.

## Open questions for the Owner

None blocking right now. Worth revisiting later: whether saved schemes
need user accounts (portfolio pattern has one project with auth —
`listing-studio` — and one deliberately without — `when-we-meet`); punt
until persistence (M5) is actually being designed.
