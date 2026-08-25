// The wire format's plain types/constants — split out from lib/wasm/index.ts
// so server-only code (lib/validation.ts, app/actions.ts) can import them
// without pulling in the browser-only wasm loader (`init`/`compute_scheme`,
// which touches `import.meta.url`-based fetch and has no business being
// evaluated in a server action's module graph). Mirrors wasm/src/lib.rs's
// WireStitch/WireScheme by hand — no shared codegen for these yet, see
// web/AGENTS.md.

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
