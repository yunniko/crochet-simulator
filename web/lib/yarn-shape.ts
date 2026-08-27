// M7: turns the flat, straight-post segments the WASM bridge returns into
// something that actually looks like folded yarn — real thickness along a
// smooth curve, and a per-stitch-kind "wiggle" standing in for the loops/
// wraps a real stitch has. Framework-free (no three.js here) so it's
// testable without a renderer; YarnViewer.tsx turns the output into
// three.js geometry.
//
// Deliberately does NOT touch core/wasm's geometry at all (GOALS.md M7's
// explicit constraint) — every function here only *reinterprets* the
// existing base/top anchor points the physics/relaxation/validation model
// already produces. The wrap counts below are a stylized approximation
// for visual distinctiveness ("does a dtr read as taller/more complex than
// a dc"), not a literal simulation of yarn-over counts — see
// `STITCH_WRAP_COUNTS`'s own comment.

import type { StitchKind, WireStitch } from "@/lib/stitch-kinds";
import type { WasmSegment } from "@/lib/wasm";

// Mirrors `crochet_core::validate::DEFAULT_YARN_DIAMETER` by hand (no
// shared codegen across the FFI boundary yet, see web/AGENTS.md) — the
// same constant the self-intersection checker itself uses, so the render
// and "is this actually touching" agree on how thick the yarn is.
export const YARN_DIAMETER = 0.15;
export const YARN_RADIUS = YARN_DIAMETER / 2;

// Wrap ("twist") count per stitch kind, used only to make the rendered
// post visually distinctive — chosen to increase monotonically with the
// model's own height ordering (stitch.rs: dc < htr < tr < dtr < trtr <
// quad_tr) so a taller stitch also reads as visually "busier," but the
// exact numbers are a stylistic choice, not derived from `pre_wraps`.
// ch/ss/mr get 0: all three are zero-height point anchors in the physics
// model (base === top), so there's no post to wiggle at all — their
// visual character comes from the connecting bridge segments around them,
// which this module leaves as plain smooth curve, not a special shape.
const STITCH_WRAP_COUNTS: Record<StitchKind, number> = {
  ch: 0,
  ss: 0,
  mr: 0,
  dc: 1,
  htr: 1.5,
  tr: 2,
  dtr: 3,
  trtr: 4,
  quad_tr: 5,
};

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

function sub(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
}
function add(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}
function scale(a: Vec3, s: number): Vec3 {
  return { x: a.x * s, y: a.y * s, z: a.z * s };
}
function length(a: Vec3): number {
  return Math.sqrt(a.x * a.x + a.y * a.y + a.z * a.z);
}
function cross(a: Vec3, b: Vec3): Vec3 {
  return { x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x };
}
function normalize(a: Vec3): Vec3 {
  const len = length(a);
  return len < 1e-9 ? { x: 0, y: 0, z: 0 } : scale(a, 1 / len);
}

/**
 * A per-stitch curve standing in for its real yarn-over/loop shape, built
 * purely from that stitch's own base/top anchor points (wherever the
 * physics model actually placed them — capacity fan-out, front/back-loop
 * offset, radial ring placement, etc. all fall out of base/top already,
 * no need to special-case any of them here). The wiggle tapers to exactly
 * zero at both ends (`Math.sin(Math.PI * t)`), so `points[0] === base` and
 * the last point `=== top` precisely — it always joins its neighbouring
 * bridge/stitch without a seam.
 */
export function buildStitchCurvePoints(base: Vec3, top: Vec3, kind: StitchKind, yarnRadius = YARN_RADIUS): Vec3[] {
  const wraps = STITCH_WRAP_COUNTS[kind];
  const direction = sub(top, base);
  const height = length(direction);
  if (wraps <= 0 || height < 1e-9) {
    return [base];
  }

  const forward = scale(direction, 1 / height);
  // Any reference not nearly parallel to `forward` gives a stable
  // perpendicular frame via two cross products — which reference doesn't
  // matter, only that it's consistently not-parallel.
  const reference: Vec3 = Math.abs(forward.z) < 0.9 ? { x: 0, y: 0, z: 1 } : { x: 1, y: 0, z: 0 };
  const right = normalize(cross(forward, reference));
  const up = cross(right, forward);

  const amplitude = yarnRadius * 1.8;
  const samplesPerWrap = 6;
  const totalSamples = Math.max(4, Math.round(wraps * samplesPerWrap));

  const points: Vec3[] = [];
  for (let i = 0; i <= totalSamples; i++) {
    const t = i / totalSamples;
    const taper = Math.sin(Math.PI * t); // 0 at the ends, 1 at the midpoint
    const angle = t * wraps * 2 * Math.PI;
    const wobble = add(
      scale(right, Math.cos(angle) * amplitude * taper),
      scale(up, Math.sin(angle) * amplitude * taper * 0.6), // flattened, not a perfect circular spiral
    );
    points.push(add(add(base, scale(forward, height * t)), wobble));
  }
  return points;
}

interface Strand {
  points: Vec3[];
  flagged: boolean;
}

const STITCH_LABEL = /^stitch\[(\d+)\]$/;
const BRIDGE_LABEL = /^bridge\[\d+->\d+\]$/;

/**
 * Turns the WASM bridge's flat segment list into smooth, coloured tube
 * strands: each `stitch[i]` run of sub-segments is replaced by that
 * stitch's own wiggle curve (via `buildStitchCurvePoints`), `bridge[...]`
 * runs stay a plain 2-point span, and consecutive runs sharing the same
 * `flagged` status are merged into one continuous strand (dropping the
 * duplicate shared point) so the tube stays visually unbroken except
 * exactly where the flagged/ordinary colour actually changes.
 */
export function buildYarnStrands(segments: WasmSegment[], stitches: WireStitch[]): Strand[] {
  interface RawGroup {
    points: Vec3[];
    flagged: boolean;
  }
  const rawGroups: RawGroup[] = [];

  let i = 0;
  while (i < segments.length) {
    const label = segments[i].label;
    let j = i;
    let flagged = false;
    while (j < segments.length && segments[j].label === label) {
      flagged = flagged || segments[j].flagged;
      j++;
    }
    const first = segments[i];
    const last = segments[j - 1];

    const stitchMatch = STITCH_LABEL.exec(label);
    let points: Vec3[];
    if (stitchMatch) {
      const index = Number(stitchMatch[1]);
      const kind = stitches[index]?.kind;
      points = kind ? buildStitchCurvePoints(first.start, last.end, kind) : [first.start, last.end];
    } else if (BRIDGE_LABEL.test(label)) {
      points = [first.start, last.end];
    } else {
      // Unrecognised label shape — render as a plain span rather than
      // dropping it silently.
      points = [first.start, last.end];
    }
    rawGroups.push({ points, flagged });
    i = j;
  }

  const strands: Strand[] = [];
  for (const group of rawGroups) {
    const lastStrand = strands[strands.length - 1];
    if (lastStrand && lastStrand.flagged === group.flagged) {
      lastStrand.points.push(...group.points.slice(1));
    } else {
      strands.push({ points: [...group.points], flagged: group.flagged });
    }
  }
  return strands;
}
