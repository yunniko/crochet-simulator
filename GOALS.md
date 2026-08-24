# Goals — crochet-sim

### G-001 · Crochet scheme simulator (3D yarn-path engine + editor) — ACTIVE
**Milestone plan approved by Owner 2026-08-24. M1 in progress.**
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
- [ ] M2 — Elasticity/relaxation: a topology-driven relaxation solve over
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
- [ ] M3 — Geometry validation: self-intersection / collision detection
      on the *relaxed* yarn path (thread-vs-thread proximity in 3D, not
      just visual/projected overlap), correctly distinguishing legitimate
      crossings (post stitches etc.) from real self-intersection. Also
      cross-checks the graph-derived stitch count against pattern-style
      `(N sts)` expectations. Deliverable: engine reports pass/fail plus
      the specific problem location for a set of known-good and known-bad
      test schemes, including at least one post-stitch case that must
      *not* false-positive.
- [ ] M4 — WASM bridge + minimal viewer: compile the core to WASM
      (wasm-bindgen), wire it into a minimal Next.js/TS app with a 3D
      viewport (three.js / react-three-fiber) rendering a hardcoded sample
      scheme end-to-end in the browser, including the relaxed (not raw)
      shape and a visible flag when M3 detects a problem. Viewport should
      be built so a future flat/2D viewing mode (deferred — see
      `docs/crochet-context.md` §6a) is a plausible addition, not a
      rewrite.
- [ ] M5 — Scheme editor UI: replace the hardcoded sample with an actual
      editor for building the insertion graph directly (add stitches,
      choose insertion targets) that live-updates the relaxed 3D render
      and geometry check as the Owner edits. Must support at least one
      non-row-based (freeform) scheme, not just conventional row/round
      patterns, to prove the editor isn't secretly row-locked.
- [ ] M6 — Persistence + deploy: save/load schemes (Postgres, matching
      portfolio pattern), then deploy following
      `E:\CLAUDE\COMPANY\INFRASTRUCTURE.md`'s standard pattern, verified
      end-to-end in a browser against the live URL.

**Progress log** (newest first):
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
