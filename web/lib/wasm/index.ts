import init, {
  compute_flat_circle_demo,
  compute_overloaded_demo,
} from "./crochet_wasm";

export interface WasmVec3 {
  x: number;
  y: number;
  z: number;
}

export interface WasmSegment {
  start: WasmVec3;
  end: WasmVec3;
  /** True if this segment is part of a self-intersection M3 flagged. */
  flagged: boolean;
  /** e.g. "stitch[7]" or "bridge[6->7]" — see crochet_core::path::SegmentOwner. */
  label: string;
}

export interface DemoResult {
  stitch_count: number;
  ok: boolean;
  violation_count: number;
  segments: WasmSegment[];
}

let initPromise: Promise<unknown> | null = null;

/**
 * Loads the WASM module exactly once, however many callers ask for it.
 * No explicit path: `init()`'s default (`new URL('crochet_wasm_bg.wasm',
 * import.meta.url)`) is what lets the bundler treat the `.wasm` file next
 * to `crochet_wasm.js` as a static asset it resolves at build time —
 * passing a `public/`-relative string instead made Turbopack fail to
 * resolve the module's own internal reference to that same file.
 */
function ensureInit(): Promise<unknown> {
  if (!initPromise) {
    initPromise = init();
  }
  return initPromise;
}

export async function loadFlatCircleDemo(): Promise<DemoResult> {
  await ensureInit();
  return compute_flat_circle_demo() as DemoResult;
}

export async function loadOverloadedDemo(): Promise<DemoResult> {
  await ensureInit();
  return compute_overloaded_demo() as DemoResult;
}
