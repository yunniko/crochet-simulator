import { describe, expect, it } from "vitest";

import type { StitchKind, WireStitch } from "@/lib/stitch-kinds";
import type { WasmSegment } from "@/lib/wasm";
import { buildStitchCurvePoints, buildYarnStrands, YARN_RADIUS } from "@/lib/yarn-shape";

function seg(label: string, start: [number, number, number], end: [number, number, number], flagged = false): WasmSegment {
  return {
    start: { x: start[0], y: start[1], z: start[2] },
    end: { x: end[0], y: end[1], z: end[2] },
    flagged,
    label,
  };
}

describe("buildStitchCurvePoints", () => {
  it("starts exactly at base and ends exactly at top, for every postable kind", () => {
    const base = { x: 1, y: 2, z: 3 };
    const top = { x: 1.5, y: 2.2, z: 5 };
    for (const kind of ["dc", "htr", "tr", "dtr", "trtr", "quad_tr"] as StitchKind[]) {
      const points = buildStitchCurvePoints(base, top, kind);
      expect(points[0]).toEqual(base);
      expect(points[points.length - 1]).toEqual(top);
      expect(points.length).toBeGreaterThan(2);
    }
  });

  it("returns just the base point for zero-height kinds (ch, ss, mr)", () => {
    const base = { x: 0, y: 0, z: 0 };
    const top = { x: 0, y: 0, z: 0 }; // matches the physics model: these are point anchors
    for (const kind of ["ch", "ss", "mr"] as StitchKind[]) {
      expect(buildStitchCurvePoints(base, top, kind)).toEqual([base]);
    }
  });

  it("handles a base/top pair that isn't axis-aligned without producing NaNs", () => {
    const base = { x: 0.3, y: -1.2, z: 0 };
    const top = { x: 0.9, y: -0.4, z: 0.1 }; // direction nearly parallel to the frame's z reference
    const points = buildStitchCurvePoints(base, top, "tr");
    for (const p of points) {
      expect(Number.isFinite(p.x)).toBe(true);
      expect(Number.isFinite(p.y)).toBe(true);
      expect(Number.isFinite(p.z)).toBe(true);
    }
  });

  it("wiggle amplitude stays small relative to yarn thickness (reads as a twist, not a spiral)", () => {
    const base = { x: 0, y: 0, z: 0 };
    const top = { x: 0, y: 0, z: 2 };
    const points = buildStitchCurvePoints(base, top, "dtr", YARN_RADIUS);
    for (const p of points) {
      const radial = Math.hypot(p.x, p.y);
      expect(radial).toBeLessThan(YARN_RADIUS * 3);
    }
  });
});

describe("buildYarnStrands", () => {
  const stitches: WireStitch[] = [
    { kind: "mr", targets: [] },
    { kind: "dc", targets: [0] },
  ];

  it("produces a single continuous strand for an ordinary, unflagged scheme", () => {
    const segments = [
      seg("stitch[0]", [0, 0, 0], [0, 0, 0]),
      seg("bridge[0->1]", [0, 0, 0], [0.5, 0, 0]),
      seg("stitch[1]", [0.5, 0, 0], [0.5, 0, 0.5]),
      seg("stitch[1]", [0.5, 0, 0.5], [0.5, 0, 1]),
    ];
    const strands = buildYarnStrands(segments, stitches);
    expect(strands).toHaveLength(1);
    expect(strands[0].flagged).toBe(false);
    // Continuous: every point flows into the next with no duplicate seam
    // beyond the merge itself.
    expect(strands[0].points.length).toBeGreaterThan(1);
  });

  it("splits into separate strands exactly where the flagged status changes", () => {
    const segments = [
      seg("stitch[0]", [0, 0, 0], [0, 0, 0], false),
      seg("bridge[0->1]", [0, 0, 0], [0.5, 0, 0], true),
      seg("stitch[1]", [0.5, 0, 0], [0.5, 0, 1], false),
    ];
    const strands = buildYarnStrands(segments, stitches);
    expect(strands.map((s) => s.flagged)).toEqual([false, true, false]);
  });

  it("uses the stitch's own kind to build its curve, not a generic default", () => {
    const tallStitches: WireStitch[] = [
      { kind: "mr", targets: [] },
      { kind: "quad_tr", targets: [0] },
    ];
    const segments = [
      seg("stitch[0]", [0, 0, 0], [0, 0, 0]),
      seg("bridge[0->1]", [0, 0, 0], [0, 0, 0]),
      seg("stitch[1]", [0, 0, 0], [0, 0, 5]),
    ];
    const dcStrands = buildYarnStrands(segments, stitches.map((s, i) => (i === 1 ? { ...s, kind: "dc" as const } : s)));
    const quadTrStrands = buildYarnStrands(segments, tallStitches);
    // A quad_tr's wiggle has far more sample points than a dc's over the
    // same base/top span (see STITCH_WRAP_COUNTS) — confirms the kind
    // actually drove which template got used, not a shared default.
    expect(quadTrStrands[0].points.length).toBeGreaterThan(dcStrands[0].points.length);
  });
});
