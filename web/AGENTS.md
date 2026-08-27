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
  drei for the 3D viewport, Prisma 7 (`@prisma/adapter-pg`, client
  generated into `generated/prisma` — regenerate with `npx prisma
  generate`) + PostgreSQL via `docker compose up -d db` (root
  `docker-compose.yml`) for saved-scheme persistence (M6). No accounts —
  a saved scheme is reached by an unguessable slug, not a login; see
  `../HANDOVER.md`'s M6 access-model decision before adding anything that
  assumes an "owner."
- `lib/wasm/` — the compiled engine. `crochet_wasm.js`/`.d.ts`/
  `crochet_wasm_bg.wasm` are **generated** by `wasm-bindgen`, not hand-
  written (see `../HANDOVER.md` M4 for the exact rebuild command) —
  excluded from ESLint in `eslint.config.mjs` for that reason. `index.ts`
  is the one hand-written file in that folder: a thin loader wrapper.
  The wire-format types/constants it re-exports (`WireStitch`,
  `STITCH_KINDS`, etc., mirroring `wasm/src/lib.rs`'s Rust DTOs by hand —
  no shared codegen for these yet) actually live in `lib/stitch-kinds.ts`,
  split out specifically so server-only code (`lib/validation.ts`,
  `app/actions.ts`) can use them without pulling the browser-only wasm
  loader into a server action's module graph.
- `dynamic(..., { ssr: false })` **cannot be called directly inside a
  Server Component** in this Next version — only from within a Client
  Component. `EditorApp` (the real app shell) needs `ssr: false` itself
  (it reads `window.location.origin` during render for the share-link
  display), so `app/EditorAppLoader.tsx` is a one-line Client Component
  wrapper that does the `dynamic()` call; `page.tsx` and `s/[slug]/
  page.tsx` (Server Components) import that wrapper, never `dynamic`
  directly. `YarnViewer`'s own `dynamic(..., {ssr:false})` inside
  `EditorApp` is fine as-is, since `EditorApp` is already a Client
  Component by the time it's called.
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
- **`lib/yarn-shape.ts` (M7, restructured M8)** turns the WASM bridge's
  flat segment list into real, thick, per-stitch-shaped tube geometry —
  pure/framework-free logic (no `three` import), consumed by
  `YarnViewer.tsx`. Rendering-layer only, deliberately: it never touches
  `core`/`wasm`'s actual geometry, only reinterprets the base/top points
  that geometry already produced. Only postable stitches (`dc` and
  taller — `core`'s `height() > 0`) get a wiggle. **`height() === 0` is
  not the same as "base equals top"** — `ch` has zero `height()` but
  real positional extent (`geometry.rs`'s `lays_out_as_line` gives it a
  genuine `CHAIN_STEP_X`-long span); only `ss`/`mr` are true zero-extent
  point anchors. An earlier version conflated the two and silently
  collapsed every all-chain scheme to invisible points — see
  `../HANDOVER.md`'s M8 entry. Chains still don't visually read as
  linked ovals (a named limitation, not an oversight — would need
  shaping bridge segments specifically, separate work). M8 also stopped
  merging strands across stitch boundaries (`buildYarnStrands` used to
  merge consecutive same-flagged runs into one mesh) — each stitch is
  now its own clickable mesh, tagged `stitchIndex: number | null`, since
  the editor needs to resolve a click back to a specific stitch.
- **If a client-component change doesn't seem to take effect in dev, even
  after a hard reload or a brand-new tab, suspect a stale Turbopack cache
  before the code.** Hit this concretely in M7: the viewer rendered
  completely blank with zero console errors after an otherwise-correct
  change; diagnostic logging proved the generated geometry data was valid
  the whole time. Stopping the dev server, deleting `.next/`, and
  restarting fixed it immediately. Worth trying early, not as a last
  resort, if a change looks right on disk but the browser disagrees.
- Two test layers, per Company standard: Playwright e2e (`tests/e2e/`,
  `npm run test:e2e`), asserting via `data-testid` hooks on the stats
  readout / stitch list, not on canvas pixels (visual correctness of the
  3D render itself needs real human/visual review) — `persistence.spec.ts`
  (M6) hits the real dev Postgres through actual server actions, not
  mocked. Vitest unit tests (`tests/unit/`, `npm run test:unit`) added in
  M6 once there was real pure-TS logic worth isolating (`lib/validation.ts`'s
  zod schema, `lib/slug.ts`'s generator); M8 added `lib/tool-placement.ts`'s
  whole interaction state machine as pure logic specifically so it could
  be tested this thoroughly (21 tests) without needing real canvas
  clicks for every case. The bulk of business logic still lives in
  `../core/`'s Rust unit tests, reached through the single
  `compute_scheme` wasm call.
- **Clicking the `<canvas>` in e2e tests (M8) needs a real readiness
  signal, not just "element is visible."** `YarnViewer.tsx`'s `Canvas`
  sets `data-r3f-ready="true"` on its own DOM element from `onCreated`
  specifically for this — react-three-fiber's raycasting/event wiring
  isn't necessarily finished the moment the element is attached and
  Playwright calls it "stable," and a click that lands before it is silently
  does nothing. `tests/e2e/helpers.ts`'s `waitForCanvasReady` waits on
  that attribute; an earlier heuristic (polling `canvas.toDataURL()` for
  a length threshold) still flaked under repeated runs. For clicking a
  *specific* rendered stitch (as opposed to "anywhere on the yarn," which
  `ch`/`mr` tolerate — see `lib/tool-placement.ts`), `clickUntil` grid-
  searches the canvas rather than trying to hand-derive the exact camera
  projection for a known 3D point — deliberately pragmatic, not elegant,
  see its own comment.
- **Test coverage gap, found and fixed in M5 — worth repeating so it isn't
  reintroduced:** a preset/test can build a scheme and assert its stitch
  count without ever asserting it *validates* (`ok`/`violation_count`).
  That gap let a genuinely self-intersecting "freeform" demo preset pass
  both its Rust and Playwright tests while actually rendering "Flagged" in
  the browser — caught only by manual visual verification. Any new preset
  or e2e spec that's meant to demonstrate a *working* scheme should assert
  `ok`/`violation_count` explicitly, not just stitch count or a target
  label — see `wire_scheme_supports_freeform_non_row_targeting` in
  `../wasm/src/lib.rs` and the matching Playwright spec for the pattern.

