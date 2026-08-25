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

**Bending/repulsion + target-capacity fix (2026-08-25, Owner-directed).**
Owner asked to fix the wide-shell folding limitation flagged above, and
provided real calibration from their own crochet experience: an ordinary
stitch fits ~7 siblings ("hard but possible"), 11 doesn't ("won't fit
physically"); a tightened magic ring cinches into a pointy 3D shape at
3-5 siblings, a flat circle at 6-8, ripples in 3D at 9+, and can't be
tightened at all far beyond that (yarn thickness); a chain/chain-space is
much more elastic (granny squares use 3, lace can call for very
different counts). Implemented as:

- **`CapacityStyle`** (`stitch.rs`, new): `Fixed` (ordinary stitches —
  small, roughly constant capacity; overflow bulges out of plane, capped
  so genuine overcrowding still trips M3), `TightenedRing` (magic ring's
  default — radius grows with count up to a flat plateau, then behaves
  like `Fixed`), `Elastic` (`ch`, or a ring explicitly left open via a
  new per-instance `StitchInstance.capacity_override` — radius keeps
  growing, no plateau, no ripple). One shared capacity threshold (7)
  serves both the ordinary-stitch and tightened-ring cases, since the
  Owner's own numbers for both sit close together.
- **New `MR` (magic ring) stitch kind.** A magic ring is a single loop of
  working yarn, not a run of chains (Owner correction, 2026-08-25) — a
  genuinely different real-world construction from `ch`, even though both
  happen to share the same *engine-level* properties (no insertion step,
  zero height) as foundation anchors. Sharing those properties is exactly
  what caused a real bug, caught by a test assertion mismatch rather than
  reasoned out in advance: `mr` initially fell into `ch`'s "lay out in a
  line" placement branch too, when it should stay a single point anchor.
  Fixed with `StitchDef.lays_out_as_line` (true only for `ch`).
- **Radial placement** (`geometry.rs`): siblings sharing a target are now
  arranged around a circle (`radius_and_wave`, angle = `2*PI*index/
  total`), not offset linearly along one axis — a straight line has no
  way to represent "opens into a wide circle" or "ripples in 3D." Needed
  a pre-pass over the whole scheme to know each target's eventual sibling
  count before placing the first sibling (`target_total`), not just a
  running counter.
- **Sibling repulsion** (`relax.rs`): every pair of stitches sharing a
  single target now gets a one-sided repulsion force, active only once
  closer than `SIBLING_REPULSION_MIN_DISTANCE` (0.3, above M3's yarn
  diameter for real margin). Directly addresses the folding gap: springs
  only ever related a stitch to its own target and its immediate
  working-order neighbour, never to siblings further round a wide fan —
  confirmed empirically that a 7-`dc` shell could relax two non-adjacent
  siblings to ~1e-16 apart without this.
- Two tuning fixes surfaced while wiring this together, both by testing
  actual numbers, not guesswork: `POST_DEPTH_OFFSET` raised 0.4→0.7 (the
  radial placement changed a post stitch's neighbours enough to reopen a
  near-miss the earlier `INCREASE_SPREAD_X` fix had closed); confirmed
  `BASE_RING_RADIUS` (0.4) didn't need to move once that was fixed.

**Verified end-to-end, not just per-piece:** a full pipeline test (raw
placement → relaxation with repulsion → M3 validation) confirms 7
siblings into one ordinary stitch validates cleanly and 11 is correctly
flagged — landing exactly on the Owner's own stated boundary, not just
passing isolated unit assertions. 42 unit tests total (was 32), clean
under `cargo clippy --all-targets`, `cargo fmt` applied.

**Known simplification, stated in `docs/crochet-context.md` §5a and
worth repeating here:** the "pointy" look for a lightly-loaded tightened
ring is currently just a narrower radius, not an actual per-stitch
inward/outward lean — a reasonable proxy, not a verified claim about the
precise silhouette. The decrease branch (multiple targets on one stitch)
also doesn't get any of this capacity/ring treatment — out of scope for
this round.

**Front/back loop and post as real geometry (2026-08-25, Owner-directed).**
Owner gave a precise description of front/back loop mechanics (front
strand faces right relative to the crocheting direction, back strand
faces left; using only one strand leaves the other genuinely free for a
different later stitch — real visual/structural consequences, not a
bookkeeping nicety) and a worked example (mosaic crochet: a back-loop-only
row, then a later row skipping it entirely to reach into the left-free
front loops with a taller stitch). `LoopTarget::FrontOnly`/`BackOnly`
already existed in the graph model but had **zero geometric effect** —
fixed by giving them a real offset (`LOOP_HALF_OFFSET` in `geometry.rs`),
on the same axis `FrontPost`/`BackPost` already used. Needed the same
"tune against a real test, don't guess" treatment as `POST_DEPTH_OFFSET`
did: an initial small value (0.2) left a mosaic-crochet-style test scheme
with a near-miss; raised to 0.5. Added as regression tests: front/back
loop siblings of one target stay geometrically distinct, and the mosaic
scheme validates without a false positive. Long-range targeting itself
(the "skip a whole row back" part of mosaic crochet) needed no new work —
already fully supported by the insertion-graph model (§4) with no
special-casing for distance. Flagged, not built: nothing yet stops two
different stitches from claiming the *same* strand of the *same* target,
which would be a real pattern error — that's a graph-consistency check,
not a geometric one, and is a different piece of work from anything here.

44 unit tests total (was 42), clean under `cargo clippy --all-targets`,
`cargo fmt` applied.

**Viewer/highlighting design intent captured (2026-08-25, Owner) —
documentation only, nothing built.** For whenever M4/M5 actually happen:
default highlighting shows a stitch's loop, or its leg/post if it has one
distinct from the loop; an opt-in detailed view highlights every point
where the folded yarn touches itself as its own part — which turns out to
map directly onto `crate::path`/`crate::validate`'s existing raw-
coincidence machinery (§8 invariant 4's "same structural point" check),
not a new concept to invent when the time comes. Also flagged: "every
hole can be a target" splits into two cases — chain-marked holes (already
fully supported, `ch` is `Elastic`) vs. holes with no stitch marking them
at all (filet-mesh-style gaps), which would need a genuinely new *derived/
virtual* target-reference concept, distinct from `StitchRef` — real
architecture work for whenever the editor's interaction model is actually
designed, not attempted now. Full writeup: `docs/crochet-context.md` §5c.

**M4 done (2026-08-25): WASM bridge + minimal viewer.** First milestone
touching `wasm/` and `web/` — nothing existed there before this.

**`wasm/` (new crate, `crochet-wasm`).** Deliberately thin: builds two
hardcoded demo schemes, runs each through the exact same core pipeline
`crochet-core`'s own tests use (raw placement → relaxation → validation),
and hands the *relaxed* yarn path plus which segments are flagged back to
JS as plain serialisable data (`serde` + `serde-wasm-bindgen`, DTOs kept
separate from `crochet-core`'s own types so the core crate stays
dependency-free). No scheme-building/editing capability — that's M5.
Two demos: `compute_flat_circle_demo` (magic ring + 6 dc round 1 — clean,
validates ok) and `compute_overloaded_demo` (15 dc into one ring — flagged,
proving the "visible flag" path actually lights up, not just the clean
path). Toolchain: `wasm32-unknown-unknown` target via `rustup target add`,
`wasm-bindgen-cli` installed via `cargo install wasm-bindgen-cli --version
<matching the wasm-bindgen crate version exactly> --locked` (version
mismatch between the crate and the CLI is a real footgun — pin both).
Rebuild command is in the root `README.md`.

**A real bug found building the demo, not a WASM/web issue at all:**
building a proper flat-circle demo (ring round 1 + a plain round-2
increase) surfaced the local-density limitation now documented in
`docs/crochet-context.md` §5a — round 2's increases, each fine against
their own target, collided with *neighbouring* increases' children. Tried
tuning constants first (several rounds — angular step, radius); each fix
shifted the problem rather than solving it, and one attempt (coupling
`Elastic`'s radius growth to the same constant `TightenedRing` uses)
silently broke `TightenedRing`'s narrow/flat calibration until a test
caught it. Landed on a real, principled fix instead of more tuning:
`Fixed` targets (ordinary increases) fan out gradually now, `Elastic`/
`TightenedRing` targets (rings, chain-spaces — genuinely isolated rounds)
always wrap the full circle regardless of size — see §5a's "Geometric
mechanism" for the full account. The cross-*target* density problem
itself (as opposed to the within-target angle problem, which this fixed)
is still open — M4's shipped demo deliberately stops at round 1 to avoid
it rather than pretending it's solved.

**`web/` (new Next.js/TS app).** Minimal viewer: a dark-themed page with
a toggle between the two demos, a react-three-fiber `<Canvas>` rendering
the relaxed yarn path as line segments (ordinary vs. flagged, different
colours), and a stats readout (stitch count, ok/flagged + violation
count) — a light echo of the "Model Statistics" panel in the Owner's UI
mockups (2026-08-25; full Blender-style layout — scene tree, properties
panel, modeling-tools stack — is real reference for M5's editor, well
beyond M4's minimal scope, not attempted here). WASM bindings live in
`lib/wasm/` (generated files + one hand-written loader/types file, see
`web/AGENTS.md`).

**Three real bugs hit and fixed while verifying this in an actual
browser** (per the Company standard: "verified by running them," not
just written and assumed working) — worth recording since none were
obvious from the app's own behaviour:
1. **Turbopack build failure** on the wasm-bindgen-generated `new
   URL('crochet_wasm_bg.wasm', import.meta.url)` call: it statically
   analyses that pattern to bundle the `.wasm` as an asset, which only
   works if the file is physically *next to* the JS glue — routing it
   through `public/` instead broke the resolution. Fixed by keeping
   `crochet_wasm_bg.wasm` alongside `crochet_wasm.js` in `lib/wasm/` and
   calling `init()` with no explicit path (its default resolution is
   exactly the pattern the bundler expects).
2. **`allowedDevOrigins`**: Next's dev-origin check silently 403s every
   JS chunk when the dev server is reached via `127.0.0.1` instead of
   `localhost` (which is how Playwright's `webServer` drives it) — the
   app doesn't error, it just hangs forever at "Computing scheme…" with
   no visible cause. Found by capturing console/network activity during
   a failing e2e run, not from the app's own behaviour. Fixed with
   `allowedDevOrigins: ["127.0.0.1"]` in `next.config.ts`.
3. **Automated screenshots came back solid black** even though the app
   was rendering correctly the whole time (confirmed via `canvas.
   toDataURL()` returning real, changing image data) — a known CDP/
   screenshot-tooling quirk with WebGL canvases that don't set
   `preserveDrawingBuffer`. Fixed by setting `gl={{ preserveDrawingBuffer:
   true }}` on the `<Canvas>`, which also happens to be generally useful
   for any future "export view" feature. Verified visually afterward:
   both demos render correctly (a small zigzag flat-circle shape; a
   denser, partly-red-highlighted overloaded ring), and `OrbitControls`
   camera rotation works.

**Testing.** Playwright e2e (`web/tests/e2e/viewer.spec.ts`, 3 specs)
asserting on the stats-readout text as a reliable proxy for "the WASM
pipeline actually ran and produced the right result" — not on canvas
pixels, since verifying the *3D render itself* looks right needs real
visual review (done manually above), not something a DOM assertion can
meaningfully check. No Vitest unit-test layer yet: there's no TS business
logic worth unit-testing until M5 adds real scheme-editing logic; all the
actual business logic so far lives in `core/`'s 44 Rust unit tests (the
`Fixed`-vs-`Elastic` angle fix was caught by existing tests, not new
ones). `web/AGENTS.md` records this reasoning so it isn't mistaken for an
oversight later.

`cargo test` (44 core + 2 wasm), `cargo clippy --all-targets`, `cargo fmt
--all`, `npm run lint`, `npm run build`, and `npm run test:e2e` (3/3) all
clean as of this milestone.

**M5 done (2026-08-25): scheme editor UI.** Replaces M4's hardcoded
demo-toggle viewer with a real editor.

**`wasm/` — general `compute_scheme` API, replacing the two M4 demo
functions.** `wasm/src/lib.rs` now exposes one `#[wasm_bindgen]` function,
`compute_scheme(wire: JsValue) -> Result<JsValue, JsValue>`, taking a
`WireScheme` (plain `kind`/`targets`/`loop_target`/`capacity_override`
JSON, mirrored by hand in `web/lib/wasm/index.ts` — no shared codegen yet,
same as M4's DTOs). `build_scheme_from_wire` validates the forward-
reference discipline the whole model relies on (§4: a target must already
be placed) and returns a clear string error — surfaced to the Owner
directly, not a panic — for a bad kind, an unknown enum value, or a
forward reference. M4's two demo builders (`build_flat_circle_scheme`,
`build_overloaded_ring_scheme`) stay as internal test fixtures (no longer
`#[wasm_bindgen]`-exported) since they're still useful known-good/known-
bad regression cases.

**`web/` — `SchemeEditor` component + preset library.**
`web/components/SchemeEditor.tsx`: a form (kind, loop target, capacity
override, a checkbox list of every existing stitch as a target candidate)
plus add/remove-last/clear, and a live `<ol>` stitch list. `app/page.tsx`
holds the `stitches` array as the one source of truth and recomputes via
`compute_scheme` on every change (a `useEffect` keyed on `stitches`,
deliberately *not* resetting `result`/`error` synchronously first — that
tripped the same `react-hooks/set-state-in-effect` ESLint rule M4 hit;
fixed the same way, by gating the empty-state UI on `stitches.length`
directly instead of on state that would need resetting). `web/lib/
presets.ts` (new) holds four starting-point schemes as plain data, each
wired to a header button: the M4 flat-circle and overloaded-ring demos
(unchanged), a 3-`tr`-into-one-chain shell, and a freeform (non-row)
example — see below.

**A real gap in test coverage, found by manual browser verification, not
by any automated check.** The freeform preset originally built to satisfy
this milestone's explicit requirement ("must support at least one
non-row-based scheme... to prove the editor isn't secretly row-locked")
was a three-way cross-link (`ch`, `tr`→0, `dc`→0, `dtr`→1, `dc`→`[2,1]`) —
structurally identical to `crochet-core`'s own `freeform_scheme_with_no_
row_structure_places_successfully` test. Both a new Rust wasm test
(`wire_scheme_supports_freeform_non_row_targeting`) and a new Playwright
spec were written for it and both passed — but neither ever asserted
`ok`/`violation_count`, only stitch count and one target's label (the
Rust test came directly from that pre-existing core test, which itself
only ever checked placement succeeds and geometry is finite — never
self-intersection — so the gap propagated forward without anyone
deliberately deciding to skip that check). Manually verifying all four
presets in a real browser (per Company standard: "changes are verified by
running them") surfaced that this preset actually renders "Flagged (7
intersections)" — a genuine self-intersection (plausibly involving the
last stitch's `[2, 1]` decrease-like cross-link, since decreases/multi-
target stitches get no capacity/ring geometric treatment — a known,
documented limitation, see above), not a bug in the flag itself. A
flagged flagship demo undermines rather than proves the milestone's own
acceptance criterion, so replaced it with a simpler, genuinely non-row
example instead: a spike stitch (`ch`, `ch`, `ch`, `dc`→2 [ordinary], `dc`
→0 [a spike, two stitches further back than its immediate predecessor]) —
validates clean, confirmed both by a strengthened assertion and by eye.
Both the Rust and Playwright tests for this preset now assert the clean
result explicitly (`assert!(result.ok)` / `expect(...).toHaveText("OK")`),
with a comment recording why, specifically so a scheme that merely
*builds* without asserting it *validates* can't pass as "proof" again.

**Testing.** Playwright e2e (`web/tests/e2e/viewer.spec.ts`, 5 specs, was
3): default preset clean, overloaded-ring flagged, the freeform spike
preset validates clean (see above), clearing and adding a stitch by hand,
remove-last. Still no Vitest layer — `SchemeEditor`'s logic is thin enough
(form state + array splice) that it doesn't yet meet the "real TS business
logic" bar `web/AGENTS.md` set as the trigger for adding one; revisit if
that changes. `cargo test` (44 core + 6 wasm), `cargo clippy
--all-targets`, `cargo fmt --all`, `npm run lint`, `npm run build`, `npm
run test:e2e` (5/5) all clean. Also manually exercised in a real browser:
all four presets (including the swapped-in freeform spike, confirmed
clean/white segments not red), plus clearing and hand-adding a stitch.

Not started: M6 (persistence + deploy) onward. Goal G-001's 6-milestone
plan is in `GOALS.md`, approved by the Owner 2026-08-24.

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

- `core/` — Rust crate: an **insertion graph** of stitch instances (working
  order + insertion-target edges — see `docs/crochet-context.md` §4,
  `graph.rs`), an extensible stitch registry (§3a, `stitch.rs`), capacity-
  aware raw placement geometry (§5a, `geometry.rs`, M1), a mass-spring
  relaxation/elasticity solve with sibling repulsion (§6, §5a, `relax.rs`,
  M2), continuous relaxed-path reconstruction (`path.rs`, M3), and
  self-intersection/count validation on that path (§8, `validate.rs`, M3).
  Pure Rust, unit-testable without any UI, no dependency on `wasm`/`web`.
- `wasm/` (M4, generalised M5) — thin `wasm-bindgen` crate: one exported
  `compute_scheme(wire)` taking whatever stitch graph the editor built (as
  plain JSON), running it through `core`'s exact pipeline, and serialising
  the relaxed path + flagged-segment info back to JS via
  `serde-wasm-bindgen`. No scheme-building logic of its own — `web/` owns
  the stitch list, this crate only ever computes.
- `web/` (M4 minimal viewer, M5 real editor) — Next.js/TypeScript app: a
  `SchemeEditor` component for building the insertion graph stitch by
  stitch (kind, targets, loop target, capacity override), a react-three-
  fiber 3D viewport of the live-relaxed shape, a stats readout, and a
  preset library (`lib/presets.ts`) of starting-point schemes including a
  non-row-based one — confirms the model/editor genuinely isn't row-locked
  per D4 below, not just in the core engine but end-to-end through the UI.

## Next steps

M6 (persistence + deploy): save/load schemes to Postgres (matching the
portfolio's existing pattern — see `E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`),
then deploy following the standard pattern there, verified end-to-end in a
browser against the live URL. Pending Owner sign-off on M5 first, per
standard milestone-boundary process.

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
