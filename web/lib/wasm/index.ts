import init, { compute_scheme } from "./crochet_wasm";

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

export interface ComputeResult {
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

export async function computeScheme(scheme: WireScheme): Promise<ComputeResult> {
  await ensureInit();
  return compute_scheme(scheme) as ComputeResult;
}

// --- Wire format (mirrors wasm/src/lib.rs's WireStitch/WireScheme by
// hand — no shared codegen for these yet, see web/AGENTS.md) ---

export type StitchKind = "ch" | "ss" | "dc" | "htr" | "tr" | "dtr" | "trtr" | "quad_tr" | "mr";

export const STITCH_KINDS: StitchKind[] = ["ch", "ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr", "mr"];

export type LoopTarget = "Both" | "FrontOnly" | "BackOnly" | "FrontPost" | "BackPost";

export const LOOP_TARGETS: LoopTarget[] = ["Both", "FrontOnly", "BackOnly", "FrontPost", "BackPost"];

export type CapacityStyle = "Fixed" | "Elastic" | "TightenedRing";

export const CAPACITY_STYLES: CapacityStyle[] = ["Fixed", "Elastic", "TightenedRing"];

export interface WireStitch {
  kind: StitchKind;
  targets: number[];
  loop_target?: LoopTarget;
  capacity_override?: CapacityStyle;
}

export interface WireScheme {
  stitches: WireStitch[];
}
