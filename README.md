# Crochet Sim

A crochet-scheme design tool that simulates the actual yarn thread — folded
and intersected stitch by stitch in 3D — so a designer can check whether a
scheme is physically possible (and see roughly how it will look) before
spending time and yarn on a real trial.

Standalone web app: a Rust simulation/geometry core (compiled to WASM for
the browser) drives a Next.js/TypeScript UI with a 3D viewport. See
**[HANDOVER.md](./HANDOVER.md)** for architecture, the stack-choice
rationale, and current state. Goal and milestone tracking:
**[GOALS.md](./GOALS.md)**. Crochet domain reference (UK/GB terminology and
the construction rules the engine is built on): **[docs/crochet-context.md](./docs/crochet-context.md)**.

## Status

M1–M5 done. `core/` (Rust) implements the insertion-graph engine — stitch
registry, raw 3D placement (capacity-aware: an ordinary stitch validates
~7 shared siblings and correctly flags ~11+; a tightened magic ring reads
as pointy at 3-5, flat at 6-8, wavy at 9+; front/back loop targeting is
geometrically real, supporting techniques like mosaic crochet), a mass-
spring relaxation solve (stitch topology, not a separate material
property, determines stretchiness), and self-intersection / stitch-count
validation on the relaxed shape. `wasm/` bridges it to the browser via
`wasm-bindgen`, exposing one general `compute_scheme` call for whatever
graph the UI builds. `web/` is a real scheme editor: add stitches, choose
insertion targets/loop targets/capacity overrides, and watch the 3D
render and validation update live — including a non-row (freeform) preset
proving the model and editor aren't secretly row-locked — see it running
with the commands below. See `GOALS.md` → G-001 for the milestone plan
and progress log, and known/deferred limitations (a dense round's several
increases can still collide with a *neighbouring* increase — flagged, not
yet fixed; decrease/multi-target stitches get no capacity/ring geometric
treatment).

## Run locally

Rust core/WASM bridge:

```bash
cargo test                                    # from the repo root
cargo clippy --all-targets
cargo fmt --all

# Rebuild the WASM bridge after changing core/ or wasm/ (needs the
# wasm32-unknown-unknown target and a matching wasm-bindgen-cli — see
# HANDOVER.md's M4 section for exact versions/install commands):
cargo build -p crochet-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/crochet_wasm.wasm \
  --out-dir web/lib/wasm --target web --typescript
```

Web viewer (`web/`):

```bash
cd web
npm install
npm run dev          # http://localhost:3000
npm run lint
npm run build
npm run test:e2e      # Playwright — starts its own dev server on :3100
```

No persistence yet — nothing saves or reloads a scheme (M6 is next). The
editor itself is real: build a scheme from scratch, or start from one of
four presets (flat circle, overloaded ring, shell, freeform spike).
