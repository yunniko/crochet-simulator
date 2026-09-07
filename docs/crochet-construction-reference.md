# Crochet construction reference — domain-expert review (2026-09-07)

Produced by the Company's `domain-expert` subagent (see `COMPANY/STANDARDS.md`
→ "Domain depth", and `GOALS.md` G-004/M1) as a cited check on
`docs/crochet-context.md` and `core/src/yarn_shape.rs`, which had never been
verified against an authoritative crochet-construction source. This doc
records that check's outcome; it supplements `crochet-context.md`, it doesn't
replace it.

**Retrieved:** 2026-09-07, via web search against secondary crochet-technique
sources (KnitPro, joyofmotioncrochet, Craft Yarn Council, oombawkadesigncrochet,
theloopylamb, thecrochetarchitect, Interweave/LoveCrafts, Edie Eckman,
crochet365knittoo, supergurumi, cosycrochetbytasha). No single canonical UK
style-guide primary source was found or checked line-by-line — the terminology
and construction-step claims below are corroborated across multiple
independent, mutually-consistent secondary sources with no contradictions
found among them, which is the strongest grounding a text-only research pass
can provide; that residual gap (no single canonical citation) matches what
`crochet-context.md`'s own intro already discloses.

## Domain reference (established, tier 1 — matches this project's implementation)

- **UK↔US terminology ladder**: `ch → ss → dc → htr → tr → dtr → trtr → quad tr`
  (UK) maps one rung below `ch → sl st → sc → hdc → dc → tr → dtr → trtr` (US).
  Confirmed independently across multiple sources with no disagreement.
- **Per-stitch mechanics** — all confirmed against tutorial/technique sources,
  including the rare `quad tr`:
  - `ss`: insert, yarn over, draw through both stitch and working loop at once — zero height.
  - `dc`(UK)/`sc`(US): insert, yarn over, draw up loop (2 on hook), yarn over, draw through both.
  - `htr`/`hdc`: yarn over before inserting, draw up loop (3 on hook), yarn over, draw through all 3 at once.
  - `tr`: yarn over once, draw up loop (3 on hook), `[yo, pull through 2]` × 2.
  - `dtr`: yarn over twice, draw up loop (4 on hook), `[yo, pull 2]` × 3.
  - `trtr`: yarn over 3×, draw up loop (5 on hook), `[yo, pull 2]` × 4.
  - `quad tr`: yarn over 4×, draw up loop (6 on hook), `[yo, pull 2]` × 5.
- **Front/back post stitches**: hook goes around an earlier tall stitch's
  vertical post/shaft (front-to-back-to-front or reverse), bypassing its top
  loops entirely.
- **Chain anatomy**: a finished chain has three visible parts — front loop,
  back loop (the "V"), and a back bump/bar — a comparatively **flat,
  ribbon-like** structure once tensioned, with a *consistent* orientation
  running down the whole chain (this is how every foundation-chain
  counting/insertion tutorial identifies "the V" and "the bump").
- **hdc/htr height quirk**: multiple independent sources note the standard
  2-chain turning-chain for hdc/htr is commonly "too tall" for the stitch's
  real height — corroborates (without fully proving) treating htr's height as
  something other than a uniform one-step-per-pre-wrap ladder.

## Findings against this project's implementation

### Tier 1 — correctly modeled, no action needed
- Terminology table and US/UK disambiguation (`crochet-context.md` lines 30–47).
- Per-stitch pre-wrap/draw-through recipe (`crochet-context.md` §3;
  `stitch.rs` lines 195–280), including `quad tr`'s `[yo, pull2]×5`.
- `ch` has zero insertion targets (`stitch.rs` `has_insertion: false` for `CH`).
- Front/back post mechanics (§2).
- The ⚠ at `crochet-context.md` line 41 isn't an open question — resolved
  correctly already; safe to downgrade from a warning to informational.

### Tier 2 — reasonable approximation; name the assumption explicitly
1. **htr height = 1.0 + 0.5×pre_wraps → 1.5** (`stitch.rs` lines 128–143).
   Better grounded than the naive linear alternative (which the "hdc turning
   chain is too tall" craft observation contradicts), but the specific "0.5"
   has no citable numeric source — it's a tuned approximation, not a measured
   fact. The code comment (lines 136–137) states it as self-evident; it isn't.
2. **Chain-as-interlocked-loop topology** (`yarn_shape.rs` lines 109–152). The
   general shape family (near-closed loop, small entry/exit gap) is
   directionally correct. The specific `CHAIN_HALF_GAP = 65°` and the
   45°-off-vertical diagonal planes are uncited visual-tuning constants
   inherited from the earlier TypeScript prototype (per the file's own header,
   lines 14–16).
3. **Post loops concentrated near shaft top** (`BAR_SPAN_START = 0.82`, lines
   158–176). Physically defensible direction (the stacked pull-throughs all
   happen near the hook), no citation for "82%" specifically.
4. **Magic-ring "pointy" capacity bands** (`crochet-context.md` §5a). Direction
   confirmed (fewer stitches/round → more coning), specific bands (3–5/6–8/9+)
   are the Owner's own calibration, already disclosed as such — keep that
   disclosure, don't let it harden into a "hard rule."

### Tier 3 — unfounded or misconception-risk; flag by name
1. **"Chain links alternate whole orientation, like a keychain" —
   `yarn_shape.rs` lines 114–121.** Overstated. Real tensioned crochet chain
   is flat/ribbon-like with *consistent* orientation (front loop/back
   loop/back bump, all facing the same way down the chain) — no source
   supports an alternating 90°-style twist per link as a real yarn feature.
   The metal-keychain analogy conflates a rigid-link system with yarn. Looks
   like a carried-over mental model from the pre-D11 rendering-layer era, not
   a construction fact. **Needs a human crochet-literate check.** Worth
   re-justifying (if kept) purely as a rendering-legibility choice — the
   comment already separately mentions avoiding an edge-on view — rather than
   as a claim about real yarn behavior.
2. **`bar_count = 2` for `htr`/`hdc` labeled "a deliberate visual choice, not
   a construction-stage count"** (`yarn_shape.rs` lines 216–227). Already
   self-disclosed correctly in the code as decorative (htr clears all 3 loops
   in one motion, not two stages) — flagging per the task's instructions, but
   no action needed beyond awareness that it visually implies a two-stage
   construction that didn't happen.
3. **Alternating left/right "twist" on tr+ posts** (`yarn_shape.rs` lines
   154–163, 195). **Contradictory sources**: some describe tall-stitch
   texture generally; others explicitly describe a visibly twisted post as a
   *sign of a technique error* (twisted hook or wrong yarn-over order), not
   the correct appearance. This could not be resolved from web sources alone
   — **needs a human crochet-literate reviewer or a real swatch comparison**
   before deciding whether to keep, soften, or remove the alternation.
4. **Numeric capacity thresholds** ("7 hard, 11 won't fit," magic-ring
   bands; `crochet-context.md` §5a, `stitch.rs`'s `CapacityStyle` doc
   comments). No craft literature quantifies these — expected and already
   disclosed as Owner calibration; keep as labeled estimates, not hardened
   rules.

## Confidence & gaps

- **Strong**: terminology ladder, per-stitch construction recipe (incl.
  `quad tr`), front/back-post mechanics. Multiple independent, consistent
  sources; no contradictions.
- **Unresolved, needs a human**: the tr+ post "twist" (item 3 above) — real
  craft sources disagree on whether this is a feature or a beginner mistake.
  This is the single highest-value follow-up from this review.
- **Unresolved, likely unresolvable by literature search**: exact tuned
  constants (65° gap angle, 0.82 span fraction, capacity thresholds) — no
  source will ever quantify these; they need either a physical-swatch
  measurement or explicit acceptance as engineering approximations (mostly
  already the case).
- **Out of scope**: physics/solver constants (`insertion_stiffness`,
  relaxation behavior) — covered by the separate rod-mechanics review
  (G-004/M2).
