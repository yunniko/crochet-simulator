# Yarn/rod mechanics reference — domain-expert review (2026-09-07)

Produced by the Company's `domain-expert` subagent (see `COMPANY/STANDARDS.md`
→ "Domain depth", and `GOALS.md` G-004/M2) as a cited check on the relaxation
solver's physics (`core/src/relax.rs`, `rod.rs`, `ccd.rs`, `geometry.rs`)
against real Discrete Elastic Rod (DER) theory and real yarn material
properties. Scope: physics/numerics only — construction/terminology is
covered separately in `docs/crochet-construction-reference.md`.

**Retrieved:** 2026-09-07, via web search and partial paper access (primary
PDFs for Bergou et al. 2008 and the 2021 DER review resisted direct text
extraction; an ar5iv HTML rendering of the review paper and search-engine
paraphrases of the primary paper were used instead — see Confidence below).

## Domain reference

**Discrete Elastic Rods (bending)**
- Discrete curvature binormal: `kb_i = 2(e_prev × e_curr) / (|e_prev||e_curr| + e_prev·e_curr)`
  — Bergou, Wardetzky, Robinson, Audoly, Grinspun, "Discrete Elastic Rods,"
  SIGGRAPH 2008 (ACM TOG 27(3)), eq. 1; confirmed consistent with the later
  review "Simple deformation measures for discrete elastic rods and ribbons"
  (Charles et al., Proc. R. Soc. A 2021, arXiv:2107.04842).
- Per-vertex bending energy `E_i = α|kb_i|²/l̄_i`, `l̄_i` the Voronoi length at
  vertex `i`. **Secondary sources disagree** on whether `l̄_i` is the sum or
  the average (half-sum) of the two adjacent edge lengths — not resolved here
  (primary PDF unreadable); flagged rather than guessed.
- Isotropic (scalar) bending stiffness is the standard, explicitly-supported
  Bergou et al. simplification for a rod with near-circular cross-section
  (vs. the general 2×2 modulus matrix needed for an anisotropic ribbon).
- Twist/material-frame energy is a first-class, normally-coupled part of the
  full DER model (Bergou et al. 2008 §4; extended in "Discrete Viscous
  Threads," SIGGRAPH 2010). Bishop (twist-free) frames via parallel transport
  are the standard mechanism for separating real material twist from frame
  bookkeeping artifacts.

**Contact — Incremental Potential Contact (IPC)**
- Barrier potential `b(d,d̂) = -κ(d/d̂ - 1)²ln(d/d̂)` for `0<d<d̂`, `C²`, zero
  at/beyond `d̂` — Li, Ferguson, Schneider, Langlois, Zorin, Panozzo, Jiang,
  Kaufman, "Incremental Potential Contact," ACM TOG 39(4), 2020.
- Typical `d̂` in the original paper's test scenes: ~0.1–1mm, a small fraction
  of scene scale — chosen for solver conditioning, not as a physically
  meaningful "contact starts here" distance.
- Edge-edge CCD via the coplanarity-cubic-root method is standard practice —
  Bridson, Fedkiw, Anderson, "Robust Treatment of Collisions, Contact and
  Friction for Cloth Animation," SIGGRAPH 2002.

**Real plied craft yarn (worsted-weight, e.g. cotton/acrylic)**
- Diameter: Craft Yarn Council Category-4 "worsted/medium" is ~9–12
  wraps-per-inch → roughly **2.5–3.5mm** effective diameter.
- Bending rigidity: measured via Peirce cantilever (ASTM D1388) or KES-FB2 in
  real textile testing; highly variable by fiber, twist, spinning method — no
  single citable number found for worsted cotton/acrylic specifically.
  **Uncited — recalled/inferred, not sourced**: order-of-magnitude estimate
  ~10⁻⁶–10⁻⁵ N·m² per ply from fiber-bundle scaling, explicitly not a
  measurement.
- Yarn-to-yarn friction coefficient: literature broadly places it in
  **0.1–1.0**, fiber- and finish-dependent, no universal constant.
- Real yarns carry residual/unbalanced ply twist, a documented driver of
  self-curling/snarling independent of bending stiffness.

## Findings

1. **Curvature binormal (`rod.rs:151–166`) — Tier 1.** Correct, literal
   transcription of Bergou et al.'s eq. 1, including the right degenerate
   fold-back fallback and unit tests verifying the right invariants. A
   genuine instance of DER, not an invented approximation.
2. **Voronoi-length normalization (`relax.rs:143–157`, line 146: average of
   the two edge lengths) — Tier 2, honestly hedged.** The module's own
   comment already calls this "Bergou et al. eq. 2-ish" — appropriately
   hedged, since secondary sources disagree on sum-vs-average and the primary
   PDF couldn't be verified directly. Practically inconsequential: the
   bending stiffness constant is untethered anyway (finding 5), so any
   missing factor-of-2 is fully absorbed into it.
3. **Isotropic scalar bending stiffness (`rod.rs:26–32`) — Tier 1/2,
   correctly justified**, not hand-waved: yarn's near-round cross-section is
   exactly the case Bergou et al. name as appropriate for the isotropic
   simplification.
4. **Finite-difference bending gradient (`relax.rs:163–201`,
   `BENDING_GRADIENT_EPS=1e-6`) — sound engineering substitution**, not a
   physics simplification: converges to the true analytic gradient with
   negligible error at yarn-scale coordinates; avoids a hand-derived
   3×3-Jacobian transcription risk. Correctly labeled in-code.
5. **Absolute stiffness constants (`CONTINUITY_STIFFNESS=0.6`,
   `BENDING_STIFFNESS=0.2`, `BARRIER_STIFFNESS=0.5`,
   `insertion_stiffness()` 0.1–0.9 in `stitch.rs:152–164`) — the key
   finding.** By the letter of "tier 3" these are ungrounded — no measured
   yarn bending rigidity, friction coefficient, or modulus behind them. But:
   - The code discloses this plainly, twice, in two different modules — not
     hidden.
   - **It isn't even well-posed to fix as stated**: the solver has no
     absolute length/mass/time unit system. Position units are relative to a
     `dc` stitch's own height (`stitch.rs:128–143`), not millimeters; there's
     no yarn-weight parameter wired to `DEFAULT_YARN_DIAMETER`
     (`validate.rs:46`, hardcoded `0.15`) despite `crochet-context.md:73`
     flagging thread diameter as a future input; no mass or physical time
     step exists. You cannot compare `BENDING_STIFFNESS=0.2` to a real yarn's
     mN·cm² rigidity without first defining what one position-unit and one
     time-unit *are* in millimeters and seconds — that mapping doesn't exist.
   - Given the project's current, already-decided scope (`relax.rs:1–11`,
     matching **D5** in `HANDOVER.md`: elasticity derives from stitch
     topology alone, "never a separate 'yarn material' parameter"), this is
     a **deliberate architectural choice consistent with an existing
     decision**, not an oversight — effectively tier 2 in practice.
   - It becomes a real gap **only if** the project's goals ever expand to
     claim yarn-weight-dependent draping (a mercerized-cotton dc swatch
     really does relax differently from a loose-spun-acrylic one of the same
     pattern in reality) — the current model structurally cannot express
     that, since `insertion_stiffness()` is a pure function of stitch
     construction, never of yarn material. Flagging as a **scope boundary**,
     not a silently-accepted gap.
6. **Contact barrier formula (`relax.rs:293–299,309–315`) — Tier 1 for form,
   Tier 2 for parameter choice.** Mathematically the same shape as real IPC's
   barrier (differs only by a constant folded into `stiffness` vs `κ`) — a
   correct instance, not a divergent one. `BARRIER_ACTIVE_DISTANCE=0.3` being
   2× `DEFAULT_YARN_DIAMETER` (far wider, relatively, than real IPC's
   0.1–1mm-vs-meter-scale ratio) is a **deliberate, well-reasoned
   compensation** for the project's own candidly-documented lack of a
   per-step CCD gate (`relax.rs:255–263`) — a wider activation zone
   substitutes for the missing hard per-step distance guarantee under plain
   explicit Euler stepping. Correctly justified.
7. **Friction is entirely absent — a real, unaddressed gap.** No
   `friction` match anywhere in `core/src`. All contact/repulsion forces are
   purely normal-direction. This matters for the domain: real crocheted
   fabric holds its shape partly *because* inter-strand friction resists
   slip at loop-crossing contact points, not from non-interpenetration
   alone. Defensible for the current goal (a static relaxation to a
   plausible non-self-intersecting rest shape, where normal repulsion alone
   likely suffices) — would matter more for any future load-bearing-drape or
   dynamic-handling feature.
8. **Twist deferral (`rod.rs:18–24`) — Tier 2, honestly staged.** Bishop-
   frame/parallel-transport code exists and is unit-tested in `rod.rs` but is
   never called from `relax.rs` (confirmed via grep — no `bishop`/
   `parallel_transport` reference outside `rod.rs`). Legitimate staged
   simplification for the current milestone bar. Worth noting: real yarn's
   residual ply twist is a documented driver of chain/single-crochet
   self-curling independent of bending — a future "does a chain curl
   realistically" feature would need this dormant code wired into a real
   twist energy term.
9. **Explicit-Euler, mass-less quasi-static integration (`relax.rs:1216–1223`,
   fixed 300-step default) — Tier 2, reasonable for the stated goal.** A
   heavily-damped relaxation closer to energy-descent than a real
   elastodynamic solver — appropriate since the deliverable is a static
   equilibrium shape, not transient dynamics. DER's own standard practice
   (implicit integration) exists specifically to handle stiffness numerically
   stiff for large steps; this project sidesteps that by tuning stiffness
   low enough for explicit stepping to stay stable, consistent with finding 5.

## Confidence & gaps

- Primary PDFs (Bergou et al. 2008, the 2021 review) resisted text
  extraction; relied on an ar5iv HTML rendering plus search-engine
  paraphrases — weaker than the primary text. The one place secondary
  sources disagreed (Voronoi-length sum-vs-average, finding 2) is flagged,
  not silently resolved.
- No specific, citable bending-rigidity number for worsted cotton/acrylic
  yarn was found — a real textile/materials expert should pull an actual
  KES-F or Peirce dataset before any future calibration attempt.
- Friction coefficient range (0.1–1.0) is a broad consensus range, not a
  single constant — fiber/finish/geometry-specific.
- Any future absolute physical calibration (findings 5, 7) is gated on this
  project having no defined length/time/mass mapping to real-world
  millimeters/seconds/grams at all — that mapping would need to exist before
  importing any real yarn property directly into a stiffness constant.
