import { describe, expect, it } from "vitest";

import type { WasmSegment } from "@/lib/wasm";
import { buildYarnStrands } from "@/lib/yarn-shape";

function seg(label: string, start: [number, number, number], end: [number, number, number], flagged = false): WasmSegment {
  return {
    start: { x: start[0], y: start[1], z: start[2] },
    end: { x: end[0], y: end[1], z: end[2] },
    flagged,
    label,
  };
}

// M14: yarn-shape.ts no longer generates any stitch shape itself — that
// real loop-through-loop geometry now comes straight from `core`
// (`crochet_core::yarn_shape`, wired through `place_scheme`/`path.rs`
// and the WASM bridge) as an ordinary run of small segments per stitch.
// This module's only remaining job is grouping those segments into
// strands, so that's all these tests cover — see `core/src/yarn_shape.rs`
// for the actual stitch-shape tests.
describe("buildYarnStrands", () => {
  it("produces one strand per contiguous label run, concatenating every segment's points in order", () => {
    const segments = [
      seg("stitch[0]", [0, 0, 0], [0, 0, 0]),
      seg("bridge[0->1]", [0, 0, 0], [0.5, 0, 0]),
      seg("stitch[1]", [0.5, 0, 0], [0.5, 0.2, 0.5]),
      seg("stitch[1]", [0.5, 0.2, 0.5], [0.5, 0, 1]),
    ];
    const strands = buildYarnStrands(segments);
    // stitch[0] (a single degenerate segment), bridge[0->1], stitch[1]
    // (two segments concatenated into one 3-point strand) — three
    // separate strands, deliberately not merged across stitch boundaries
    // (M8: clicking a mesh has to resolve to a specific stitch index).
    expect(strands).toHaveLength(3);
    expect(strands.map((s) => s.stitchIndex)).toEqual([0, null, 1]);
    expect(strands.every((s) => !s.flagged)).toBe(true);
    expect(strands[2].points).toEqual([
      { x: 0.5, y: 0, z: 0 },
      { x: 0.5, y: 0.2, z: 0.5 },
      { x: 0.5, y: 0, z: 1 },
    ]);
    // Endpoints still coincide across strands, so the un-merged tubes
    // still read as one continuous piece of yarn visually.
    expect(strands[0].points[strands[0].points.length - 1]).toEqual(strands[1].points[0]);
    expect(strands[1].points[strands[1].points.length - 1]).toEqual(strands[2].points[0]);
  });

  it("tags each strand's own flagged status independently, not merged with neighbours", () => {
    const segments = [
      seg("stitch[0]", [0, 0, 0], [0, 0, 0], false),
      seg("bridge[0->1]", [0, 0, 0], [0.5, 0, 0], true),
      seg("stitch[1]", [0.5, 0, 0], [0.5, 0, 1], false),
    ];
    const strands = buildYarnStrands(segments);
    expect(strands.map((s) => s.flagged)).toEqual([false, true, false]);
  });

  it("flags a strand if any of its constituent segments were flagged", () => {
    const segments = [
      seg("stitch[1]", [0, 0, 0], [0, 0, 0.5], false),
      seg("stitch[1]", [0, 0, 0.5], [0, 0, 1], true),
    ];
    const strands = buildYarnStrands(segments);
    expect(strands).toHaveLength(1);
    expect(strands[0].flagged).toBe(true);
  });

  it("never assigns a stitch index to a bridge strand", () => {
    const segments = [
      seg("stitch[0]", [0, 0, 0], [0, 0, 0]),
      seg("bridge[0->1]", [0, 0, 0], [0.5, 0, 0]),
      seg("stitch[1]", [0.5, 0, 0], [0.5, 0, 1]),
    ];
    const strands = buildYarnStrands(segments);
    const bridge = strands.find((s) => s.stitchIndex === null);
    expect(bridge).toBeDefined();
    expect(bridge!.points).toEqual([
      { x: 0, y: 0, z: 0 },
      { x: 0.5, y: 0, z: 0 },
    ]);
  });
});
