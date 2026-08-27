# Goals — crochet-sim

### G-001 · Crochet scheme simulator (3D yarn-path engine + editor) — ACTIVE
**All 7 planned milestones (M1-M7) done as of 2026-08-28 — pending Owner
review/sign-off against the acceptance criteria below before moving to
Completed.**
- **What:** A web app where a designer builds a crochet scheme (stitch
  types, rows, chains) and sees a simulated 3D yarn path for it — the
  thread folded and intersected the way real yarn would be — with
  automatic geometry checks that flag physically impossible stitch
  sequences, so schemes can be validated without a physical trial.
- **Why:** Save time and yarn on trial-and-error when designing complex
  crochet patterns; let the designer catch impossible geometry before
  ever picking up a hook.
- **Acceptance criteria:** Owner can, in a browser, enter a stitch
  sequence, see it rendered as a 3D yarn path that visually matches how
  the real stitches would sit, get a clear flag when a sequence is
  geometrically impossible (with an example the Owner tries and confirms
  looks right/wrong appropriately), save and reload a scheme, and the app
  is deployed and reachable the same way the portfolio's other projects
  are (see `E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`).
- **Constraints:** None from the Owner beyond the stack decisions already
  logged in `HANDOVER.md` (Rust→WASM core, Next.js/TS UI, standalone web
  app not a Blender plugin, full 3D simulation for the MVP — not a 2D-only
  first cut). No deadline given.

**Milestones** (revised 2026-08-24 per Owner decisions — see
`docs/crochet-context.md` and `HANDOVER.md` D4–D7; proposed, pending Owner
sign-off before M1 starts):
- [x] M1 — Core data model: Rust crate implementing the **insertion graph**
      (working-order stitch sequence + insertion-target edges — not
      rows/rounds as structural objects) and an **extensible stitch
      registry** seeded with the basic UK ladder (ch, ss, dc, htr, tr, dtr,
      trtr, quad tr) via the pre-wrap/draw-through recipe. Produces raw 3D
      point/segment yarn-path coordinates for a given stitch graph,
      including increases/decreases/spike stitches/freeform placement as
      ordinary cases (not special-cased). Pure Rust, unit tests only (no
      UI/WASM/relaxation yet) — tests prove basic sanity (consistent
      segment length, no degenerate/NaN geometry, correct stitch count for
      both conventional row/round schemes and a non-row freeform one). The
      top-level scheme object is a **list of threads** (each thread one
      insertion graph) from the start, even though M1 only ever populates
      it with one thread — multi-thread schemes/joins are deferred (see
      `docs/crochet-context.md` §4a) but must be additive later, not a
      restructuring.
- [x] M2 — Elasticity/relaxation: a topology-driven relaxation solve over
      the insertion graph (each insertion-target edge as a constraint with
      some give, not a rigid offset) that settles raw M1 placement into a
      physically plausible relaxed shape, and can re-solve under an
      applied stretch. Deliverable: visibly different (and directionally
      correct) relaxed shapes for a dense-stitch swatch vs. an open/tall-
      stitch swatch, and a stretch-response demo for at least one test
      scheme. Solver design should leave room for an optional later
      planar constraint (2D/3D construction-space modes, deferred — see
      `docs/crochet-context.md` §6a) rather than assuming unconstrained-3D
      is the only mode it will ever run in.
- [x] M3 — Geometry validation: self-intersection / collision detection
      on the *relaxed* yarn path (thread-vs-thread proximity in 3D, not
      just visual/projected overlap), correctly distinguishing legitimate
      crossings (post stitches etc.) from real self-intersection. Also
      cross-checks the graph-derived stitch count against pattern-style
      `(N sts)` expectations. Deliverable: engine reports pass/fail plus
      the specific problem location for a set of known-good and known-bad
      test schemes, including at least one post-stitch case that must
      *not* false-positive.
- [x] M4 — WASM bridge + minimal viewer: compile the core to WASM
      (wasm-bindgen), wire it into a minimal Next.js/TS app with a 3D
      viewport (three.js / react-three-fiber) rendering a hardcoded sample
      scheme end-to-end in the browser, including the relaxed (not raw)
      shape and a visible flag when M3 detects a problem. Viewport should
      be built so a future flat/2D viewing mode (deferred — see
      `docs/crochet-context.md` §6a) is a plausible addition, not a
      rewrite.
- [x] M5 — Scheme editor UI: replace the hardcoded sample with an actual
      editor for building the insertion graph directly (add stitches,
      choose insertion targets) that live-updates the relaxed 3D render
      and geometry check as the Owner edits. Must support at least one
      non-row-based (freeform) scheme, not just conventional row/round
      patterns, to prove the editor isn't secretly row-locked.
- [x] M6 — Persistence + deploy: save/load schemes (Postgres, matching
      portfolio pattern), then deploy following
      `E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`'s standard pattern, verified
      end-to-end in a browser against the live URL.
- [x] M7 — Realistic yarn rendering: render the yarn with real cylindrical
      thickness (the yarn-diameter constant the validator already uses,
      not a flat line) along smooth draping curves, and give each stitch
      kind its own parametric curve template (the actual loop/wrap shape
      a real stitch has, not the straight-post abstraction the physics/
      validation geometry uses) so stitches visually read as themselves.
      Rendering-layer only — must not require changing core/wasm's
      relaxation or validation geometry, which stay the physics
      abstraction they already are underneath the visual overlay.

**Progress log** (newest first):
- 2026-08-28 — **M7 done — realistic yarn rendering.** New
  `web/lib/yarn-shape.ts` (pure, Vitest-tested): real cylindrical
  thickness along a smooth Catmull-Rom curve (radius mirrors
  `DEFAULT_YARN_DIAMETER`, so the render and the self-intersection
  checker agree on yarn thickness), plus a per-stitch-kind "wiggle"
  standing in for each postable stitch's (`dc` through `quad_tr`) real
  loop/wrap shape — built purely from that stitch's own base/top anchor
  points, so it works correctly under capacity fan-out, front/back-loop
  offset, and radial ring placement without special-casing any of them.
  Named limitation: `ch`/`ss`/`mr` (zero-height point anchors in the
  physics model) get no wiggle, so chains don't yet visually read as
  linked ovals — would need shaping the *bridge* segments between two
  chains specifically, a separate piece of work, not attempted here.
  Hit a real debugging detour: the viewer rendered completely blank with
  no errors after the first implementation — diagnostic logging proved
  the geometry data was valid the whole time; the actual cause was a
  stale Turbopack dev-server cache, fixed by clearing `.next/`. Verified
  by hand in a real browser across all four presets after the fix.
  `npm run test:unit` (17/17, was 10), `npm run lint`, `npm run build`,
  `npm run test:e2e` (8/8, unchanged) all clean; `cargo test`/clippy/fmt
  unaffected (rendering-only, no core/wasm changes, as required). Full
  account in `HANDOVER.md`.
- 2026-08-27 — **M6 done — deployed.** Live at
  **https://crochet.app.craftodejnice.cz**. Owner created a new public
  GitHub repo (`github.com/yunniko/crochet-simulator`) and asked to push
  — GitHub rejected it over an email-privacy mismatch between this
  machine's git identity and the Owner's verified GitHub email; Owner
  chose to rewrite all 12 local commits' author/committer email rather
  than change GitHub's setting (nothing had been pushed anywhere yet, so
  safe). A second real bug surfaced only on the actual live deploy (not
  caught by the earlier local Docker build): `web/public/` was untracked
  by git (empty directories aren't tracked), so a genuine fresh clone had
  no `public/` at all and the Dockerfile's `COPY` for it failed outright
  — fixed with a tracked `.gitkeep`, redeployed clean. Verified for real:
  the live HTTPS site computes schemes correctly and a save/reload
  round-trips through the live Postgres, and every other site/container
  on the shared host (`when-we-meet`, `listing-studio`, `parley`, Grafana)
  was confirmed undisturbed (`docker ps` uptimes unchanged, all still
  respond). DNS needed no work (wildcard already covered the subdomain).
  Full account, including the exact detours, in `HANDOVER.md`.
- 2026-08-25 — **M6 persistence done; deploy half blocked on Owner input.**
  Owner resolved the standing "accounts?" open question: no accounts,
  unguessable private links (each saved scheme gets a 12-char slug;
  whoever has the link can view/re-save it, nothing listed publicly) —
  same model `when-we-meet` uses for rooms, minus its participant-
  identity layer (nothing here needs one). Added Postgres + Prisma
  (`web/prisma/schema.prisma`: one `Scheme` model storing the wire-format
  stitch list as JSON, per D2's original "schemes are documents" note),
  a `saveScheme` server action, and a `/s/[slug]` route that loads a
  saved scheme into the same editor. Docker image + compose file follow
  `when-we-meet`'s exact shape (app port 30020, Postgres port 54322 —
  next free in each range, not yet verified on the live host). Verified
  for real: production build outside Docker first (isolating "does the
  wasm asset resolve outside dev mode" from Docker itself, given this
  project's history of environment bugs), then a from-scratch `docker
  compose --profile app up -d --build`, with save/reload exercised in a
  real browser against the containerized app + its migrated database, not
  just the dev server. `npm run test:unit` (new, 10 tests: slug
  generation, save-schema validation), `npm run test:e2e` (8, was 5: 3 new
  persistence specs against the real dev Postgres). Full account in
  `HANDOVER.md`.
  **Deploy half not started** — this project has never been pushed to
  GitHub (`git remote -v` empty), and both creating a repo and the live-
  server steps are always-escalate actions per `OPERATIONS.md`. Open
  question logged in `HANDOVER.md` for the Owner: create a repo (where,
  what visibility) and confirm the subdomain/port picks before deploying
  for real.
- 2026-08-25 — Owner asked what realistic yarn rendering (real thickness,
  stitches that visually read as themselves) would need. Answer: cheap
  part is rendering-layer only (tube geometry at the existing
  `DEFAULT_YARN_DIAMETER`, a spline instead of a raw polyline); the real
  work is that every stitch is currently modelled internally as a
  straight post (a physics/validation abstraction, `core/src/geometry.rs`)
  with no actual loop/wrap shape, so looking like a real `dc` etc. needs a
  per-stitch-kind parametric curve template layered on top as a rendering
  overlay, not a change to the underlying physics geometry. Owner asked
  for this as its own milestone after M6 — added as M7 above. Continuing
  M6 (persistence + deploy), already approved and in progress.
- 2026-08-25 — **M5 done.** Replaced M4's hardcoded demo toggle with a
  real editor (`web/components/SchemeEditor.tsx`): add a stitch by
  picking kind/loop-target/capacity-override and checking earlier
  stitches as targets, remove-last/clear, a live stitch list. The wasm
  bridge grew a general `compute_scheme(wire)` API (`wasm/src/lib.rs`)
  replacing M4's two hardcoded demo functions — it takes whatever the
  editor built (as plain JSON, validated for forward-reference target
  discipline) and runs it through the same core pipeline, live on every
  edit. Four presets ship as starting points, one of them (`Freeform
  spike (non-row)`) built specifically to satisfy this milestone's
  freeform-scheme requirement: a `dc` at index 4 targets index 0, two
  stitches further back than its immediate predecessor — not "the row
  below" — and validates clean (`OK`, verified both by an assertion and
  by eye in a real browser).
  **A real gap in test coverage, caught by manual verification, not by
  any automated check:** the freeform preset originally chosen (a
  three-way cross-link: `dc` targeting `[2, 1]`) rendered as "Flagged (7
  intersections)" in the browser — a genuine self-intersection, not a
  UI bug — despite both its Rust wasm test and its Playwright e2e spec
  passing, because neither ever asserted `ok`/`violation_count`, only
  stitch count and one target label. A flagged flagship demo undermines
  the very thing M5's acceptance criterion asks it to prove, so swapped
  it for the simpler spike-stitch example above and strengthened both
  tests to assert a clean result explicitly, with a comment recording
  why, so this class of gap can't recur silently. Full account in
  `HANDOVER.md`.
  `cargo test` (44 core + 6 wasm), clippy, fmt, `npm run lint`, `npm run
  build`, `npm run test:e2e` (5/5) all clean; the shipped state was also
  exercised by hand in a real browser (all four presets, plus adding/
  removing stitches manually), not just via the test suite. Next: M6
  (persistence + deploy) — pending Owner sign-off on M5 first, per
  standard milestone-boundary process.
- 2026-08-25 — **M4 done.** New `wasm/` crate (`crochet-wasm`) and `web/`
  (Next.js/TS minimal viewer): a demo toggle, a react-three-fiber 3D
  viewport rendering the relaxed yarn path, a stats readout — the whole
  `core` → WASM → browser pipeline proven end-to-end, verified visually
  in a real browser (not just tests): a clean flat-circle demo and a
  deliberately overloaded, correctly-flagged ring demo both render and
  interact (camera orbit) correctly. Building the demo surfaced a real
  bug in M1/M2's placement (siblings sharing a target always wrapped a
  full circle regardless of size, letting a dense round's increases
  collide with *neighbouring* increases) — fixed by making `Fixed`
  targets fan out gradually while `Elastic`/`TightenedRing` targets keep
  wrapping the full circle (they represent an isolated round, not an
  embedded increase); full account and the still-open cross-target
  density limitation in `HANDOVER.md` and `docs/crochet-context.md` §5a.
  Also hit and fixed three real bugs verifying in-browser: a Turbopack
  build failure (wasm asset resolution), Next's `allowedDevOrigins`
  silently hanging the app under Playwright, and automated screenshots
  reading a stale WebGL buffer (`preserveDrawingBuffer`) — none obvious
  from the app's own behaviour, all recorded in `HANDOVER.md`/
  `web/AGENTS.md` so they aren't rediscovered from scratch. Playwright
  e2e (3 specs) added; Vitest deliberately not yet (no TS business logic
  to unit-test until M5). `cargo test` (44 core + 2 wasm), clippy, fmt,
  `npm run lint`, `npm run build` all clean. Next: M5 (scheme editor UI)
  — should also prioritise the cross-target density limitation before an
  Owner can hit it directly by building a real multi-round piece.
- 2026-08-25 — Owner corrected a documentation inaccuracy: a magic ring
  is a single loop of working yarn, not formed from chains — a genuinely
  different real-world construction from `ch`, despite the two sharing
  engine-level properties (no insertion step, zero height) as foundation
  anchors. Several passages (`docs/crochet-context.md` §4/§5a,
  `stitch.rs`, `HANDOVER.md`) worded the resemblance ambiguously enough
  to risk reading as "magic ring is chain-like" — reworded to state the
  engine-property overlap is coincidental, not evidence of shared
  construction. No code/behaviour changes, no test changes; all 44 tests
  still pass.
- 2026-08-25 — Owner described viewer/highlighting intent for M4/M5
  (default: loop or leg/post; detailed opt-in: every self-touch point of
  the folded yarn as its own highlightable part) and flagged that "every
  hole between threads can be a target" — split in the doc into
  chain-marked holes (already supported) vs. unmarked holes (filet-mesh
  style, would need a new derived/virtual target concept). Documentation
  only — `docs/crochet-context.md` §5c — nothing built, no viewer exists
  yet.
- 2026-08-25 — Owner gave a precise description of front/back loop
  mechanics (front strand faces right relative to crocheting direction,
  back faces left; using one strand leaves the other genuinely free for a
  different later stitch) plus a mosaic-crochet worked example (back-loop
  row, then a later row skipping it to reach into the left-free front
  loops with a taller stitch). Found `LoopTarget::FrontOnly`/`BackOnly`
  had zero geometric effect despite existing in the model since M1 —
  fixed with a real offset on the same axis front/back post already
  uses, tuned (0.2→0.5) against the mosaic scheme as a real test, not
  guessed. Long-range targeting itself needed no new work (already
  supported). 44 unit tests total, clean, fmt applied. Full account in
  `HANDOVER.md`.
- 2026-08-25 — Owner asked to fix the wide-shell relaxation-folding
  limitation, with real calibration from their own crochet experience
  (an ordinary stitch: ~7 comfortable, 11 won't fit; a tightened magic
  ring: 3-5 pointy, 6-8 flat, 9+ wavy, far more physically impossible;
  chain/chain-space: much more elastic). Added `CapacityStyle` (Fixed/
  Elastic/TightenedRing) to the stitch registry, a new `MR` stitch kind,
  radial (not linear) placement for siblings sharing a target, and
  explicit sibling repulsion in the relaxation solver. An end-to-end
  pipeline test confirms 7 into one stitch validates and 11 is flagged —
  matching the Owner's own boundary exactly. 42 unit tests total, clean,
  fmt applied. Full account in `HANDOVER.md`.
- 2026-08-24 — Owner asked whether lace (many stitches sharing one
  insertion point) validates correctly — it didn't, fully. Found and
  fixed a real bug: M3's adjacency rule excluded *any* two stitches
  sharing a target from checking against each other entirely, not just
  against the shared target — confirmed this let shell siblings pinned
  ~0.01 apart pass silently. Replaced with a raw-placement point-
  coincidence rule (`path.rs` now carries raw endpoints alongside relaxed
  ones). Also found and fixed a second, more basic issue this surfaced:
  `INCREASE_SPREAD_X` (0.3) was too small for *any* stitch taller than
  `dc` sharing a target with a sibling — an ordinary 2-stitch increase,
  not lace-specific — putting it right at the edge of the yarn-diameter
  threshold; raised to 0.5. **Still open, not fixed**: wide multi-way
  shares (~5+ stitches into one point, common in lace shells) can fold
  onto themselves during M2's relaxation, since the spring model has no
  bending/repulsion resistance — M3 correctly flags the resulting
  overlap, but this means wide shells don't validate cleanly yet. Full
  account in `HANDOVER.md`. 32 unit tests total, clean under clippy, fmt
  applied.
- 2026-08-24 — **M3 done.** Added `core/src/path.rs` (reconstructs each
  thread's complete relaxed yarn path, including the "bridge" segment
  between consecutive stitches whenever they don't already coincide — a
  gap M1/M2 never modelled explicitly) and `core/src/validate.rs`
  (`check_self_intersections` + `check_round`, the `(N sts)` self-check).
  Also added a real depth offset for front/back post stitches in
  `geometry.rs`, so a post stitch's path genuinely doesn't occupy the
  same space as what it reaches past — no special-casing needed in the
  checker itself. The adjacency rule (what counts as "expected to touch,
  not a collision") took two wrong attempts before landing on a 1-hop
  neighbourhood-overlap rule — full account in `HANDOVER.md`, worth
  reading before touching this code again. Verified against: an ordinary
  multi-row swatch (no false positives), the milestone's required
  front-post-stitch case (no false positive), and a deliberately
  engineered bad case — two unrelated stitches pinned to the same point —
  correctly flagged. 30 unit tests total, clean under clippy, fmt
  applied. Next: M4 (WASM bridge + minimal viewer) — first milestone
  touching `web/`/TypeScript.
- 2026-08-24 — **M2 done.** Added `core/src/relax.rs`: a mass-spring
  relaxation solver over the M1 insertion graph. Every insertion-target
  edge and every working-order continuity edge (new — the physical yarn
  strand between consecutive stitches in a thread) is a Hookean spring;
  stiffness comes from a new `StitchDef::insertion_stiffness()` (dc
  stiffest, taller/open stitches progressively softer, `ch` loosest) so
  elasticity is purely a function of stitch kind, per the Owner's original
  instruction. `RelaxationParams.pinned` provides both "hold an edge" and
  "apply a stretch" via the same mechanism. Verified: an already-rest
  swatch barely moves (idempotency), pinned positions hold exactly, and —
  the real deliverable — pulling a `dc` row's last stitch drags its free
  neighbour markedly less (1.42 units under a pull of length 3) than the
  same pull on `tr` (1.70) or `dtr` (1.76), checked against actual printed
  numbers, not just a passing assertion. 22 unit tests total, clean under
  clippy, fmt applied. Solver works on plain `Vec3` positions with no
  z-axis special-casing, so it doesn't foreclose the deferred §6a planar
  constraint. Next: M3 (geometry validation).
- 2026-08-24 — **M1 done.** Built `core/` (Rust, Cargo workspace at repo
  root) implementing the insertion graph: `stitch.rs` (open registry
  seeded with the basic UK ladder), `graph.rs` (`Scheme` = `Vec<Thread>`
  per D9, `StitchInstance.targets: Vec<StitchRef>` per D4/D5/D10),
  `geometry.rs` (raw 3D placement — no relaxation, that's M2). 17 unit
  tests, all passing; `cargo clippy --all-targets` clean; `cargo fmt`
  applied. Tests explicitly cover a conventional row-into-chain scheme
  and a fully non-row freeform scheme (both required by the M1
  acceptance criteria above), plus increase/decrease/spike-stitch/
  unplaced-target-error cases. Git repo initialized for the project;
  work committed in two steps (docs scaffold, then M1 core). Next: M2
  (elasticity/relaxation).
- 2026-08-24 — Owner approved the 6-milestone plan. Starting M1.
- 2026-08-24 — Owner corrected a model detail: `ch` has **zero** insertion
  targets (formed purely from the working loop), and a turning chain is
  not structurally special — it's just an ordinary chain; the earlier ⚠
  flag implying turning chains need per-convention special-casing was
  wrong and has been removed. Updated `docs/crochet-context.md` §3/§4/§8
  invariant 2 and logged as `HANDOVER.md` D10. No milestone-plan impact
  beyond making M1's data model slightly simpler than drafted (one fewer
  ambiguous case to handle).
- 2026-08-24 — Owner added a further requirement: eventual **multiple
  threads/starting points, connected later** (e.g. Irish crochet motifs
  joined after being worked separately, amigurumi parts sewn onto a body).
  Confirmed deferred/out of scope now, must be addable later without a
  redesign. Logged as `HANDOVER.md` D9 and `docs/crochet-context.md` §4a,
  distinguishing live "crochet joins" (an insertion-target edge crossing
  threads) from after-the-fact "sewn seams" (a weaker attachment
  constraint, not a stitch). Updated §8 invariant 1 so yarn continuity is
  a per-thread invariant, not a whole-scheme one. Added a note to M1 above:
  the scheme object is a list of threads from the start, even with only
  one thread populated early on. Milestone count unchanged (still 6, still
  pending Owner sign-off).
- 2026-08-24 — Owner added a further requirement: eventual **2D/3D
  construction-space modes** — flat pieces (doilies, squares, lace) whose
  topology aligns in 2D vs. volumetric pieces (amigurumi, bowls, bags)
  whose topology aligns in 3D. Confirmed deferred/out of scope now, must
  be addable later without a redesign. Logged as `HANDOVER.md` D8 and
  `docs/crochet-context.md` §6a, with a working (non-final) assumption
  that this is one relaxation solver with an optional planar constraint,
  not two separate physics engines — revisit for real at M2 or M4/M5.
  Added forward-compat notes to M2 and M4 above; milestone count unchanged
  (still 6, still pending Owner sign-off).
- 2026-08-24 — Owner resolved the open questions from `docs/crochet-
  context.md` and corrected the core model: (1) stitch-name recognition
  should eventually cover multiple languages, not just US/UK — registry
  needs canonical IDs + mapping layers, no timeline yet; (2) gauge/tension
  stays out of scope for now, may return later; (3) fabric **elasticity
  must be simulated, as a property of stitch topology, not yarn
  material** — added as its own milestone (relaxation/solve over the
  insertion graph) rather than folded into geometry validation;
  (4) textured/compound stitches confirmed deferred, but the stitch set
  must be an extensible registry so they can be added later without a
  redesign; (5) **rows/rounds are not real simulation objects** — corrected
  the core model to a working-order insertion graph so freehand/freeform
  and hyperbolic crochet (which don't work in rows) are supported natively
  rather than special-cased. Rewrote `docs/crochet-context.md` §3a–§8
  accordingly and logged D4–D7 in `HANDOVER.md`. Milestone plan revised
  from 5 to 6 milestones (see above) — **proposed, not yet approved** by
  Owner; do not start M1 until sign-off.
- 2026-08-24 — Wrote `docs/crochet-context.md`: UK/GB crochet terminology
  and construction rules for the engine to build on (stitch anatomy as a
  pre-wrap/draw-through recipe, rows/rounds/turning chains, increase/
  decrease as insertion-point fan-in/fan-out, and the geometric invariants
  M2's self-intersection checker needs). Compiled from general UK crochet
  convention, not a single cited source — flagged ⚠ in a few places
  (turning-chain-counts-as-stitch convention; post-stitch vs. true
  self-intersection distinction) as needing real crochet-literate review
  before M1 locks in the stitch primitives. Not yet reviewed by Owner.
- 2026-08-24 — Goal created and milestone plan written. Owner chose,
  when asked: standalone web app (not Blender plugin), full 3D yarn
  simulation for MVP (not 2D-first), and left compiled-core language
  choice to JulAI (picked Rust→WASM — rationale in `HANDOVER.md` D2).
  Project scaffolded: `README.md`, `HANDOVER.md`, this file. Not yet
  reported to Owner for milestone-plan sign-off — do that before starting
  M1 per `E:\CLAUDE\COMPANY\OPERATIONS.md` step 2.
