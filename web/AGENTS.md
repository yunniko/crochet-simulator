<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->

# crochet-sim web — project conventions

Read `../HANDOVER.md` first: current state, decision record, and how this
directory fits into the whole project (`../core/` and `../wasm/` are the
actual simulation engine — this is only the viewer). Goal/milestone plan:
`../GOALS.md`. Company-wide standards: `E:\CLAUDE\COMPANY\`.

- Stack: TypeScript, Next.js App Router, Tailwind v4, react-three-fiber +
  drei for the 3D viewport. No database/Prisma yet — nothing here persists
  anything (that's M6).
- `lib/wasm/` — the compiled engine. `crochet_wasm.js`/`.d.ts`/
  `crochet_wasm_bg.wasm` are **generated** by `wasm-bindgen`, not hand-
  written (see `../HANDOVER.md` M4 for the exact rebuild command) —
  excluded from ESLint in `eslint.config.mjs` for that reason. `index.ts`
  is the one hand-written file in that folder: a thin loader wrapper, plus
  the `DemoResult`/`WasmSegment`/`WasmVec3` types mirroring `wasm/src/
  lib.rs`'s Rust DTOs by hand (no shared codegen for these yet).
- `allowedDevOrigins: ["127.0.0.1"]` in `next.config.ts` is required for
  Playwright (which drives the dev server via `127.0.0.1`) — without it,
  Next's dev-origin check silently 403s every JS chunk and the app just
  hangs at its loading state with no visible error. Don't remove it.
- Any `<Canvas>` (react-three-fiber) needs `gl={{ preserveDrawingBuffer:
  true }}` — without it the page still renders correctly on screen, but
  automated/CDP-based screenshot tooling reads a stale, cleared buffer and
  comes back solid black. Confirmed this concretely verifying M4's viewer.
- react-three-fiber touches `window` on import — any component using it
  must be loaded via `next/dynamic(..., { ssr: false })`, never imported
  directly into a server component or a client component that might SSR.
- Two test layers, per Company standard — but currently only one is set
  up: Playwright e2e (`tests/e2e/`, `npm run test:e2e`), asserting on the
  stats readout text as a reliable proxy for "the WASM pipeline actually
  ran," not on canvas pixels (visual correctness of the 3D render itself
  needs real human/visual review). No Vitest unit-test setup yet — there's
  no meaningful TS business logic to unit-test until M5 adds real scheme-
  editing logic; the actual business logic so far lives entirely in
  `../core/`'s 44+ Rust unit tests. Add Vitest when that changes, matching
  `listing-studio`/`when-we-meet`'s setup rather than inventing a new one.

