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

All 8 planned milestones (M1–M8) done. **Live at
https://crochet.app.craftodejnice.cz.**
`core/` (Rust) implements the insertion-graph engine — stitch registry,
raw 3D placement (capacity-aware: an ordinary stitch validates ~7 shared
siblings and correctly flags ~11+; a tightened magic ring reads as pointy
at 3-5, flat at 6-8, wavy at 9+; front/back loop targeting is
geometrically real, supporting techniques like mosaic crochet), a mass-
spring relaxation solve (stitch topology, not a separate material
property, determines stretchiness), and self-intersection / stitch-count
validation on the relaxed shape. `wasm/` bridges it to the browser via
`wasm-bindgen`, exposing one general `compute_scheme` call for whatever
graph the UI builds. `web/` is a direct-manipulation editor: the app
starts with a plain starting piece of yarn; pick a stitch-kind tool from
the palette and click the render to place it (empty space works for
chains; a target-requiring stitch is placed by clicking its target
directly — one click, placed immediately — or, with "Decrease mode"
toggled on, by clicking several targets in turn and confirming with the
tool button, to build a decrease). The yarn renders live as real, thick,
per-stitch-shaped 3D tubes (not flat lines — see `HANDOVER.md`'s M7
entry) with validation updating alongside it, and saving gives back an
unguessable link to reload/share the scheme by — no accounts, see
`HANDOVER.md`'s M6 access-model decision. See `GOALS.md` → G-001 for the
milestone plan and progress log, and known/deferred limitations (a dense
round's several increases can still collide with a *neighbouring*
increase — flagged, not yet fixed; decrease/multi-target stitches get no
capacity/ring geometric treatment; chains don't yet visually read as
linked ovals in the renderer; a pending-target highlight can be visually
masked when a bridge segment happens to retrace a stitch's own path;
closing a chain into a ring with a slip stitch now actually pulls, but
doesn't yet bow into a clean circle — see `HANDOVER.md`'s 2026-08-28
entry for the real fix that's still needed).

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
# Postgres for local dev (from the repo root):
docker compose up -d db

cd web
npm install
cp .env.example .env      # then edit if you changed the db port/creds
npx prisma migrate dev    # creates/applies migrations against the local db
npm run dev                # http://localhost:3000
npm run lint
npm run test:unit          # Vitest
npm run build
npm run test:e2e           # Playwright — starts its own dev server on :3100,
                            # uses the same local db (persistence.spec.ts)
```

Production-style Docker build (from the repo root, needs `web/lib/wasm/*`
already rebuilt/committed — see above):

```bash
docker compose --profile app up -d --build   # db + migrate + app, http://127.0.0.1:30020
```

Pick a tool and click the render to build a scheme from scratch, or start
from one of four presets (flat circle, overloaded ring, shell, freeform
spike); Save gives back an unguessable `/s/<slug>` link that reloads (or
re-saves over) that exact scheme — no accounts, see `HANDOVER.md`'s M6
access-model decision.
