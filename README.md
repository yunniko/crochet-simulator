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

M1–M3 done: a Rust crate (`core/`) implementing the insertion-graph
engine — stitch registry, working-order thread(s), raw 3D placement
geometry, a mass-spring relaxation solve where stitch topology (not a
separate material property) determines how stretchy the fabric is, and
self-intersection / stitch-count validation on the relaxed shape (correctly
distinguishing a legitimate post-stitch crossing from a real collision).
No WASM bridge or UI yet — see `GOALS.md` → G-001 for the milestone plan
and progress log.

## Run locally

```bash
cargo test          # from the repo root — runs core/'s unit tests
cargo clippy --all-targets
cargo fmt --all
```

No web app yet — that lands in M4 (WASM bridge + minimal viewer).
