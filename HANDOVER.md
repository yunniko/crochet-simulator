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

**M6 done (2026-08-27): persistence + deploy.** Live at
**https://crochet.app.craftodejnice.cz**.

**Access model: no accounts, unguessable private links (Owner decision,
2026-08-25).** The open question logged back at M1 ("do saved schemes
need user accounts?") came due once persistence was actually being
designed. Owner chose the simplest of three options offered (no accounts
+ unguessable links, vs. a single shared public list, vs. full accounts
matching `listing-studio`'s auth stack): each saved scheme gets a 12-char
unguessable slug; whoever has the link can view and re-save over it; no
login, no session, nothing listed publicly. This is architecturally
identical to `when-we-meet`'s room-link model (same `nanoid`
`customAlphabet`, same unambiguous 32-symbol alphabet, same collision-
retry pattern) minus that project's participant/cookie-identity layer
entirely — a saved scheme has no "owner" to distinguish from any other
visitor, so there's nothing for a cookie to identify.

**Persistence: Postgres + Prisma, JSON-document storage, matching D2's
original note.** `web/prisma/schema.prisma`'s one model, `Scheme`
(`id`, `slug` unique, `name` optional, `stitches: Json`, timestamps),
stores the exact `WireStitch[]` shape the wasm bridge already consumes —
never normalized into per-stitch rows, since nothing ever queries into
individual stitches server-side, only loads/stores the whole graph (D2
flagged this back at project kickoff: "schemes are documents, not
relational data"). Stack matches `when-we-meet` exactly (Prisma 7 with the
`@prisma/adapter-pg` driver adapter, client generated to
`generated/prisma`, `prisma.config.ts`, `lib/prisma.ts`'s singleton-across-
hot-reload pattern) — the only Company project so far using Postgres
without accounts, so the closer analog to copy was `when-we-meet`, not
`listing-studio`.

**Server actions, not a REST-style route handler.** `app/actions.ts`'s
`saveScheme` is called directly from the client component with a plain
JS object argument (Next.js server actions accept arbitrary serializable
args from a client-component call site, not only `<form action>`), so no
hand-rolled API route/fetch/JSON-body plumbing was needed. Validates via
a hand-written `zod` schema (`lib/validation.ts`) mirroring the wire
format — deliberately a *shape* check only (stitch kind is one of the 9
known enum values, targets are non-negative integers, etc.), not the
forward-reference semantic check `wasm/src/lib.rs`'s
`build_scheme_from_wire` already does: that check only matters when
actually *computing* a scheme, and the editor already runs every stitch
through `compute_scheme` before a save is ever offered, so re-deriving it
server-side would just duplicate logic that already lives correctly in
Rust. The zod layer exists purely as defence-in-depth against a request
that bypasses the editor UI entirely (a raw POST), not as the scheme's
real correctness check.

**No accounts means no ownership check on save** — `saveScheme` will
overwrite any `slug` it's given, and a `slug` that doesn't exist falls
back to creating a new scheme rather than erroring, since a stale or
copied link has no owner identity to mismatch against anyway. This is a
direct, deliberate consequence of the access-model decision above, not an
oversight — flagged here so it isn't "fixed" into an ownership check later
without revisiting that decision first.

**Routing:** `/` is the empty/preset-starting editor; `/s/[slug]` is a
server component that loads a saved scheme via Prisma and hands it to the
same client component (`EditorApp`) as initial props, `notFound()`-ing on
a miss (Next's default 404 page — no custom one, matching `when-we-meet`,
which doesn't have one either). Saving updates the slug in place via
`history.replaceState`, not a full navigation/redirect — the WASM module
stays loaded, the editor state doesn't reset.

**A real Next-16 breaking change hit while wiring this up, not a
persistence bug per se:** `dynamic(..., { ssr: false })` is no longer
allowed directly inside a Server Component in this Next version (it was
fine in `page.tsx` for M4/M5's `YarnViewer`, called from *within* the
already-client `EditorApp`/`ComputePane` — the failure is specifically
about calling it from a Server Component). `EditorApp` needs `ssr:false`
too now (it reads `window.location.origin` directly during render for the
share-link display, which would otherwise create a genuine server/client
hydration mismatch on `/s/[slug]` — a saved scheme already has a slug on
its very first render). Fixed with a one-line Client Component wrapper
(`app/EditorAppLoader.tsx`) that does the `dynamic(...)` call itself, so
the Server Component pages (`page.tsx`, `s/[slug]/page.tsx`) only ever
import that wrapper, never call `dynamic` themselves.

**Docker image.** `web/Dockerfile` + root `docker-compose.yml` follow
`when-we-meet`'s exact shape (`deps`/`build`/`runtime` stages, one-shot
`migrate` service via `prisma migrate deploy`, `--profile app` opt-in) —
picked app port 30020 and Postgres port 54322 (next free in each range
per `E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`'s registry; **not yet verified
free on the live host**, that's a live-deploy-time check, not a docs-time
one). Deliberately does **not** touch the Rust/WASM toolchain — the image
build assumes `web/lib/wasm/*` (wasm-bindgen's generated output) is
already committed and current, same assumption the project has run on
since M4. `next.config.ts` gained `output: "standalone"` (required for
the Dockerfile's `.next/standalone` copy) and the same clickjacking-
prevention security headers `when-we-meet` sets, since this project is
about to be deployed too.

**Verified for real, not just built:** ran the actual production
(`next build` + `next start`, i.e. no `next dev`) server locally outside
Docker first — specifically to isolate "does the WASM asset resolve
outside dev mode" from "does Docker also work," given this project's
history of environment-specific bugs (M4's three). Confirmed the wasm
binary is correctly traced into `.next/standalone/.next/static/media/`
and the app computes/renders correctly. Then built and ran the actual
`docker compose --profile app up -d --build` stack end-to-end and,
in a real browser: loaded the containerized app, saved a scheme (name +
Save button + share-link display all worked), and reloaded that exact
`/s/<slug>` URL fresh to confirm the round trip through the containerized
app and its migrated Postgres, not just the dev server. Also verified the
404 path for an unknown slug. Playwright e2e grew a third spec file
(`tests/e2e/persistence.spec.ts`, 3 specs: save round-trip, overwrite-in-
place on repeat save, 404 on an unknown slug) against the real local dev
Postgres — not mocked, since the point is proving the browser -> server
action -> Prisma -> Postgres path actually works. Also added a Vitest
unit-test layer (`tests/unit/`, 10 tests: `saveSchemeSchema`'s
accept/reject cases, `generateSchemeSlug`'s alphabet/length/uniqueness) —
crosses the bar `web/AGENTS.md` set for "real TS business logic worth
isolating" that M4/M5 hadn't reached yet.

**A real test-timing bug, found by running the suite repeatedly, not by
reading it.** The "saving twice overwrites in place" e2e spec failed
deterministically (3/3 repeats) with the reload showing 1 stitch instead
of 2, which looked at first like a real persistence bug (an update
silently not applying). Checked the actual Postgres row directly
(`docker exec ... psql`) to settle it: every affected test run's row *did*
have the correct 2 stitches — the DB and `saveScheme` were correct.  The
bug was in the test: unlike the *first* save (where the URL genuinely
changes from `/` to `/s/<slug>`, giving Playwright's auto-retrying
assertion something real to wait on), a *second* save to an
already-saved scheme doesn't change the URL or the client-side stitch
count as a visible side effect of that specific save completing — both
already show the post-save values before the second save's network round
trip even starts. So `expect(page).toHaveURL(...)` and
`expect(stat-stitches)` passed immediately without actually waiting for
the second save, and the following `page.goto(firstUrl)` sometimes raced
ahead of the write committing. Fixed by waiting on the actual save
request/response (`page.waitForResponse` matching the POST) before
asserting anything post-save — confirmed stable across 5 repeats after
the fix. Worth remembering for any future "does a repeat action's
completion" test: if the assertion's target value doesn't actually change
as a result of the specific action being tested, the assertion isn't
proof that action finished.

`cargo test` (44 core + 6 wasm, unaffected by this milestone), `npm run
lint`, `npm run test:unit` (10/10), `npm run build`, `npm run test:e2e`
(8/8, 5 existing + 3 new, each re-run and stable) all clean, plus the
from-scratch Docker build and manual browser verification above.

**A second real bug, found only by deploying to the actual live host, not
by the local Docker build.** `web/public/` exists locally (empty, from
the original `create-next-app` scaffold) but was never tracked by git —
git doesn't track empty directories — so the local Docker build (which
builds from the working tree, not a fresh clone) never surfaced this. A
genuine fresh `git clone` on the server has no `public/` directory at
all, and `web/Dockerfile`'s `COPY --from=build /app/public ./public`
fails outright (`"/app/public": not found`) rather than silently doing
nothing. Fixed with a tracked `web/public/.gitkeep` placeholder. Worth
remembering: this project's local Docker verification (M6, above) tests
"does the Dockerfile work against my working tree," not "does it work
against exactly what's in git" — the two only diverge on untracked files
like this one, but when they do, only a real fresh-clone deploy catches
it.

**Deployed for real (2026-08-27), following
`E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`'s standard pattern.** This project
had never been pushed anywhere — required two Owner-authorized detours
before the deploy itself could happen, both logged here since they'll
recur for any future Company project set up the same way:
1. **GitHub repo.** Publishing/pushing is always-escalate (`VALUES.md`).
   Owner created `github.com/yunniko/crochet-simulator` (public, empty)
   and asked to push to it. Push was rejected by GitHub's email-privacy
   protection — every local commit was authored with a different email
   (`nikonorova@email.cz`) than the one already verified/public on the
   Owner's GitHub account (`12hv89@gmail.com`, the email `when-we-meet`/
   `listing-studio`'s already-pushed history uses). Owner explicitly
   chose to rewrite history (`git filter-branch --env-filter`, all 12
   commits, nothing had been pushed anywhere yet so this was safe) rather
   than change GitHub's setting. **The local machine's *global* git
   `user.email` is still the mismatched one** — every future commit in
   *any* project on this machine will hit the same rejection until the
   Owner either updates it themselves or fixes it on GitHub's side; not
   changed here, since git config changes are the Owner's call, not
   JulAI's, per the charter.
2. **Live-server steps.** Confirmed via SSH that ports 30020/54322 were
   genuinely free (not just per the doc's table), cloned over HTTPS into
   `/var/www/repositories/crochet-simulator`, wrote `.env`, ran `docker
   compose --profile app up -d --build` — hit the `web/public/` bug above
   on the first attempt, fixed and re-deployed clean on the second. The
   nginx vhost + `certbot` step needed root, so it was handed to the
   Owner as an exact file + command list (a `sudo tee`-based copy, after
   a first attempt using a heredoc the Owner's terminal couldn't paste
   cleanly — worth remembering: prefer writing the file server-side via
   SSH to a path the Owner can `sudo cp` from, over a heredoc they have to
   paste themselves). DNS needed no work — `crochet.app.craftodejnice.cz`
   already resolved to the host before any of this, via the same kind of
   pre-existing wildcard `doily.app.craftodejnice.cz` already used.

**Verified end-to-end against the live HTTPS URL, not just a health-check
ping**: loaded the real site, confirmed the WASM pipeline computes
correctly, saved a scheme (name + link both worked) and reloaded that
exact `/s/<slug>` URL fresh to confirm the save round-tripped through the
live Postgres. Then confirmed every *other* site/container on the shared
host was undisturbed: `docker ps` showed identical uptimes for
`when-we-meet-*`, `listing-studio-*`, `parley-*`, `hbbs`/`hbbr` before
and after, and `meet.app.julienika.cz`, `craftale.eu`, and
`grafana.julienika.cz` (401 — auth-gated, not down) all still respond.

Not started: M7 (realistic yarn rendering, added 2026-08-25 — see
GOALS.md). Goal G-001's milestone plan is in `GOALS.md`, approved by the
Owner 2026-08-24 (M1-M6) and 2026-08-25 (M7 added).

**M7 done (2026-08-27/28): realistic yarn rendering.** Rendering-layer
only, as required — no change to `core/`/`wasm/`'s geometry, relaxation,
or validation at all this milestone.

**`web/lib/yarn-shape.ts` (new, pure/framework-free, Vitest-tested).**
Two pieces, matching the split M7's Owner-facing scoping conversation
landed on (see GOALS.md's progress log entry from when M7 was created):
- **Real thickness + smooth curves** (the "cheap" half): the WASM
  bridge's flat `WasmSegment[]` list is one continuous polyline per
  thread already (confirmed by reading `path.rs`'s construction order —
  every segment's start coincides with the previous one's end, bridges
  included) — `buildYarnStrands` walks it once, grouping consecutive
  same-label segments (`stitch[i]` / `bridge[a->b]`) and merging adjacent
  groups that share the same `flagged` status into one strand. Each
  strand becomes one `THREE.CatmullRomCurve3` + `TubeGeometry` in
  `YarnViewer.tsx`, radius `YARN_RADIUS` — mirrors
  `crochet_core::validate::DEFAULT_YARN_DIAMETER` by hand (same pattern
  as every other FFI-boundary constant/type in this project, see
  `web/AGENTS.md`), so the render and the self-intersection checker agree
  on how thick the yarn actually is.
- **Per-stitch-kind "wiggle" template** (the real work): `dc`/`htr`/`tr`/
  `dtr`/`trtr`/`quad_tr` (the postable kinds — the only ones with real
  height, `core`'s `height() > 0`) each get a small twisting curve
  (`buildStitchCurvePoints`) standing in for their actual yarn-over/loop
  shape, built purely from that stitch's own base/top anchor points —
  works correctly under capacity fan-out, front/back-loop offset, radial
  ring placement, etc. without special-casing any of them, since it only
  ever reasons about the real base→top vector the physics model already
  produced. Wrap count scales with the model's own height ordering (1 for
  `dc` up to 5 for `quad_tr`) purely for visual distinctiveness — **a
  stylized approximation, explicitly not a literal simulation of
  yarn-over counts**, documented in the module so it isn't later mistaken
  for one. `ch`/`ss`/`mr` are zero-height point anchors in the physics
  model (base === top) and get no wiggle at all — their visual character
  comes from the plain connecting bridge curve around them, a real,
  named scope limit (see below), not an oversight.

**Known limitation, stated plainly rather than glossed over:** chains
don't visually read as the small linked ovals real chain stitches are —
they render as a smooth connecting curve, same as any bridge. Building
that would mean shaping the *bridge* segments specifically between two
`ch`/`ch` stitches, not a per-stitch template (bridges are shared
connective tissue between any two stitches, not owned by one kind) — a
different, separable piece of work, not attempted this milestone given
`ch`/`ss`/`mr` sequences are a smaller fraction of most real schemes than
the postable stitches this milestone does cover well.

**A real debugging detour, worth recording so it isn't repeated:** after
first implementing this, the viewer rendered a completely blank canvas
with no console errors, across a hard reload and a brand-new tab —
looked exactly like a code bug. Added temporary diagnostic logging
(`console.log` inside the geometry-building `useMemo`) and confirmed the
generated strand/geometry data was completely valid (correct point
counts, a sane bounding sphere) — the geometry pipeline was never broken.
The actual cause: a stale Turbopack dev-server cache serving an old
compiled bundle despite the saved file being correct on disk. Fixed by
stopping the dev server, deleting `.next/`, and restarting clean.
**If a change to a client component silently doesn't seem to take effect
in dev, even after a hard reload, suspect the Turbopack cache before the
code** — this cost real time verifying (correctly) that the new code was
right before realizing the *served* code wasn't the new code.

**Verified for real**, not just built: manually exercised all four
presets in a real browser after the cache fix — the flat circle and
overloaded ring (with its red/flagged coloring correctly distributed
across the overcrowded shares), the shell (visibly taller, more
twisted `tr` posts next to a flat, un-wiggled `ch`), and the freeform
spike (the spike `dc`'s reach back to stitch 0 clearly visible as a
distinct, real strand). `npm run lint`, `npm run test:unit` (17/17, was
10 — 7 new `yarn-shape.spec.ts` tests: curve continuity at both
endpoints, zero-height kinds collapse to a point, non-axis-aligned
base/top pairs don't produce NaNs, wiggle amplitude stays yarn-thickness-
scaled, strand merging/splitting behaves correctly around flagged
boundaries, and kind actually drives which template gets used), `npm run
build`, `npm run test:e2e` (8/8, unchanged — none of them assert on
canvas pixels, by design, see `web/AGENTS.md`) all clean from a fully
clean `.next`/Turbopack cache. `cargo test`/`clippy`/`fmt` unaffected, as
expected for a rendering-only milestone.

**M8 done (2026-08-28): direct-manipulation editor.** Owner gave a
detailed spec, mid-session, for a completely different primary
interaction model — not signing off on M1-M7, but redirecting: replace
the dropdown/checkbox "Add stitch" form with a tool palette (one button
per stitch kind) where the active tool determines what clicking the 3D
render does. Also said explicitly, mid-spec: implement it as "thoughtful
inner application changes... instead of patches healing only [a]
singular part of [the] problem" — treated as a standing instruction
(saved to memory), not just guidance for this one milestone: redesign the
whole affected system coherently rather than bolt a click handler onto
the existing structure. This shaped the scope below — the M7 rendering
grouping, the app's default state, and the whole `SchemeEditor`/
`EditorApp` component boundary all changed, not just "add an onClick."

**One real design gap in the Owner's spec, resolved before building:**
decreases (a stitch sharing multiple targets) weren't addressed — asked
via `AskUserQuestion`, Owner chose "click each target in turn, click the
active tool again to confirm" over deferring decreases entirely. Two
other calls made without asking (lower-stakes, stated here rather than
interrupting for them): loop-target/capacity-override stayed as secondary
modifier toggles rather than being dropped (real, tested M5 capability);
the old form was fully replaced, not kept alongside the new flow.

**`lib/tool-placement.ts` (new, pure, 21 Vitest tests) — the whole
interaction model as a state machine**, deliberately factored out of any
React/three.js code so the actual rules live in one tested place:
- `isToolAvailable(kind, stitchCount)`: `mr` only with zero stitches
  placed (docs §5a: a magic ring is a foundation anchor only); every
  target-requiring kind only once at least one stitch exists; `ch`
  always (`docs/crochet-context.md` §3/§4: it never has a target).
- `selectTool`/`clickStitch`/`clickEmptySpace`: the three events the
  whole model reduces to. Clicking a stitch with a target-requiring tool
  active toggles it into/out of a pending-target set (not just adds —
  mis-clicks need to be undoable); clicking the *same already-active*
  tool button again either confirms a placement (pending targets
  non-empty) or deselects the tool (pending empty) — a single-target
  placement is exactly this flow with one target click before
  confirming, so no separate code path was needed for the "common case."
  `ch`/`mr` ignore *what* was clicked (they never have a target either
  way) — any click, on existing geometry or empty space, places one.

**`components/YarnViewer.tsx` restructured for per-stitch clickability.**
M7's `buildYarnStrands` merged consecutive same-flagged-status segments
into one continuous tube mesh purely for visual smoothness — fine when
nothing needed to distinguish *which* stitch a click landed on. M8 needs
exactly that, so strands are no longer merged across stitch boundaries at
all; each stitch (and each bridge) is its own mesh, tagged with
`stitchIndex: number | null` (`null` for a bridge — connective tissue
between two stitches, never itself a click target). Endpoints still
coincide exactly across strands (the wiggle tapers to zero at both ends,
per M7), so the un-merged tubes still read as one continuous piece of
yarn visually — the merge was a rendering nicety, not something the
visual result depended on. Added: a plain starting-yarn-stub mesh shown
whenever `stitches.length === 0` (nothing to compute yet, so no wasm
round trip for it), a `PENDING_COLOR` highlight for stitches in the
current pending-target set, and `Canvas`'s `onPointerMissed` wired to the
empty-space-click event. `ComputePane` (`EditorApp.tsx`) now mounts
`YarnViewer` unconditionally instead of swapping it for placeholder text
below `stitches.length > 0` — the render *is* how building a scheme
starts now, not just how an already-built one is displayed, so it can't
disappear during the empty state.

**The app's default state is now empty**, not the flat-circle preset —
matches the Owner's "the app is starting with a straight end piece of
yarn rendered" literally. Presets remain available as alternate starting
points via the header buttons, unchanged in shape; they just aren't the
*default* anymore.

**A real bug, inherited from M7, found only because M8 made an
all-chain scheme an actual exercised path.** `ch` has no wiggle template
(`STITCH_WRAP_COUNTS.ch === 0`) but *does* have real positional extent —
`geometry.rs`'s `lays_out_as_line` gives every chain a genuine
`CHAIN_STEP_X`-long base-to-top span, unlike `ss`/`mr` where base and top
truly coincide. `buildStitchCurvePoints` conflated the two ("no wiggle"
with "zero extent"), collapsing every chain to an invisible single point
whenever `wraps <= 0` regardless of its real span. M7's own manual
verification never caught this because every scheme it happened to check
had a postable stitch nearby providing enough visible geometry to make
the collapsed chains an unnoticed gap; M8's very first real test (three
chains, nothing else, built from scratch by clicking) rendered
completely blank. Diagnosed the same way as the M7 blank-canvas incident
— temporary logging to confirm the *data* was fine before suspecting the
*code* — except this time the data logging is what revealed the actual
bug (zero-length point arrays), not a stale cache. Fixed: only collapse
to a point when `height < 1e-9` (base and top genuinely coincide);
`wraps <= 0` alone now returns the real `[base, top]` span. Two new
regression tests lock this in (`ch` with real extent keeps its span; `ch`
with base===base still collapses correctly in the genuine degenerate
case).

**A second, narrower rendering limitation found and *not* fixed, logged
honestly rather than papered over:** when a target-requiring stitch
targets something other than its immediate working-order predecessor (a
spike-stitch-like case), the connecting bridge segment can retrace
*exactly* back over a stitch's own already-rendered path — e.g. `ch,
ch, dc -> [0]`: the bridge from stitch 1 (working-order predecessor) to
the `dc` (which targets stitch 0, not stitch 1) runs the literal reverse
of stitch 1's own span. Both tubes are geometrically correct and
independently coloured correctly, but they perfectly z-fight/overlap, so
a pending-target highlight on the underlying stitch can be visually
masked by the ordinary-coloured bridge drawn over the same space. Traced
with the same base/top-point logging used for the M7/M8 chain bug, this
time confirming the *rendered data* was correct on both sides — a real,
if narrow, visual-only edge case (only occurs for spike-stitch-like
divergence between working order and targeting), not a functional bug:
the click still correctly registers and the stitch list is always
correct regardless of what's visually distinguishable in a crowded spot.

**Also fixed while building this: a stale-stats display bug.** Clearing
a loaded scheme left the *previous* scheme's stats panel on screen,
including a `Flagged (N intersections)` reading for a now-empty scheme
that has nothing to be flagged — `ComputePane`'s effect deliberately
never resets `result` during a recompute (avoids flicker while editing a
non-empty scheme, unchanged since M5/M6), but that reasoning doesn't
apply once the scheme itself is gone. Fixed by gating the stats panel's
render on `stitches.length > 0 && result`, not `result` alone — no state
reset needed, so it doesn't reopen the `react-hooks/set-state-in-effect`
issue M4/M5 already hit and worked around.

**Two real e2e-test-only bugs, found by actually running the suite
repeatedly, not by reading it — same discipline as M6's "saving twice"
race:**
1. A helper (`placeChains`) re-clicked the `ch` tool button on every
   call, even when it was already the active tool — which, per the
   state machine above, *deselects* an already-active tool with nothing
   pending, not a no-op. Broke any test placing chains across more than
   one call. Fixed: only click the tool button if it isn't already
   `aria-pressed`.
2. A canvas click fired immediately after `page.goto` can land on a
   `<canvas>` that Playwright considers "visible and stable" but that
   react-three-fiber hasn't actually finished wiring raycasting up to
   yet — intermittently (not every run) silently did nothing. An initial
   fix (poll `canvas.toDataURL()` for a length threshold) still flaked
   under repeated runs, since even a freshly-cleared canvas serializes to
   more bytes than a casually-chosen threshold accounts for — a heuristic
   that merely made the race *narrower*, not gone. Replaced with a real
   signal: `YarnViewer`'s `Canvas` sets `data-r3f-ready="true"` on its own
   DOM element from `onCreated`, which only fires once r3f's
   renderer/event setup has genuinely finished; tests wait on that
   attribute instead of guessing. Stable across 60 repeated runs (6× the
   full 10-test suite) after the fix, where the `toDataURL` heuristic had
   still failed roughly 1 run in 15.
3. Clicking a *specific* rendered stitch (not just "anywhere on the
   yarn," which `ch`/`mr` tolerate) needs a screen point that actually
   lands on that stitch's mesh — hand-deriving the exact camera
   projection for a known 3D point was judged not worth the real-math
   risk (a subtly wrong constant would silently mis-click and be hard to
   notice). Used a small grid-search helper (`clickUntil`) instead: try
   points across the canvas until a caller-supplied condition is met.
   Pragmatic, not elegant — documented as such in the helper itself.

**Verified for real**, manually in a browser, the entire flow end to
end before writing a single e2e test for it: empty start → `ch`/`mr`
palette-only → first stitch via clicking the stub → `mr` disabling
itself the moment a stitch exists → `ch` placing via both a render-hit
and a genuine empty-space miss → `dc` accumulating two pending targets
(highlighted) → confirming a real decrease (`dc -> [0, 1]`) → `Remove
last`/`Clear` correctly dropping stale pending-target state. Then wrote
the e2e coverage to match what had already been confirmed working by
hand, not the other way around.

`cargo test`/`clippy`/`fmt` unaffected (no core/wasm changes this
milestone either). `npm run lint`, `npm run test:unit` (41/41, was 17 —
21 new `tool-placement.spec.ts` tests plus a handful of `yarn-shape.ts`
regression additions), `npm run build`, `npm run test:e2e` (10/10, was
8 — rewritten for the new interaction model, stable across 60 repeats)
all clean.

Not started: nothing currently planned — M8 was added mid-session on top
of the original 7-milestone plan; whatever's next is the Owner's call.
See GOALS.md for the full milestone list and what's still open
(persistence/deploy's standing git-email config note, the cross-target
local-density and decrease/multi-target-stitch geometric limitations, the
chain-visual-shape and bridge-overlap-highlighting limitations above).

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

None blocking right now.

**Standing, not urgent**: the local Windows machine's global git
`user.email` (`nikonorova@email.cz`) doesn't match the email verified on
the Owner's GitHub account (`12hv89@gmail.com`) — every future commit,
in any project, will hit GitHub's email-privacy push rejection until the
Owner fixes one side or the other (see the M6 deploy account above for
the full story). Not JulAI's to change unilaterally.

Resolved: whether saved schemes need user accounts — see the access-model
decision above (no accounts, unguessable links), 2026-08-25. Resolved:
GitHub hosting and the live deploy — see M6 above, 2026-08-27.
