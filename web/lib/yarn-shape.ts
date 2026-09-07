// M7 (restructured M13, simplified M14): turns the WASM bridge's flat
// segment list into per-strand point lists for `YarnViewer.tsx` to turn
// into three.js tube geometry.
//
// M14: `core`'s own `place_scheme`/`path.rs` now build each stitch's real
// yarn-loop path directly (`crochet_core::yarn_shape` — a chain link's
// own loop, a post's shaft with one real loop per pull-through stage),
// so the segments WASM sends already trace that real geometry point by
// point. This module used to *re-derive* a shape from just a stitch's
// base/top and its kind (M7-M13, entirely client-side, never actually
// checked against anything) — now it just renders whatever `core` already
// computed and validated, one strand per contiguous run of same-owner
// segments. No shape-generation logic belongs here any more: if a stitch
// looks wrong, the fix is in `crochet_core::yarn_shape`, not here.

import type { WasmSegment } from "@/lib/wasm";

// Mirrors `crochet_core::validate::DEFAULT_YARN_DIAMETER` by hand (no
// shared codegen across the FFI boundary yet, see web/AGENTS.md) — the
// same constant the self-intersection checker itself uses, so the render
// and "is this actually touching" agree on how thick the yarn is.
export const YARN_DIAMETER = 0.15;
export const YARN_RADIUS = YARN_DIAMETER / 2;

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

// A strand tagged with which stitch (if any) it's the shape of — M8 needs
// this: clicking a rendered mesh has to resolve back to a specific stitch
// index so it can become a target, which a merged-across-stitch-boundaries
// mesh (M7's original approach) can't support. `stitchIndex` is `null` for
// a bridge — the connecting strand between two stitches, not owned by
// either one, and never a valid click target itself.
export interface Strand {
  points: Vec3[];
  flagged: boolean;
  stitchIndex: number | null;
}

const STITCH_LABEL = /^stitch\[(\d+)\]$/;

/**
 * Turns the WASM bridge's flat segment list into one strand per
 * contiguous run of same-`label` segments (a stitch's own real loop path,
 * or a bridge's plain span) — deliberately *not* merged across stitch
 * boundaries the way M7's first version did, since each stitch needs to
 * stay its own clickable mesh (see `Strand.stitchIndex`). Every strand's
 * endpoints still coincide exactly with its neighbours' (each segment run
 * starts exactly where the last one ended, by construction in `core`), so
 * the un-merged tubes still read as one continuous piece of yarn visually.
 */
export function buildYarnStrands(segments: WasmSegment[]): Strand[] {
  const strands: Strand[] = [];

  let i = 0;
  while (i < segments.length) {
    const label = segments[i].label;
    const points: Vec3[] = [segments[i].start];
    let flagged = false;
    let j = i;
    while (j < segments.length && segments[j].label === label) {
      flagged = flagged || segments[j].flagged;
      points.push(segments[j].end);
      j++;
    }
    const stitchMatch = STITCH_LABEL.exec(label);
    const stitchIndex = stitchMatch ? Number(stitchMatch[1]) : null;
    strands.push({ points, flagged, stitchIndex });
    i = j;
  }

  return strands;
}
