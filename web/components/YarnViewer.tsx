"use client";

import { OrbitControls } from "@react-three/drei";
import { Canvas } from "@react-three/fiber";
import { useMemo } from "react";
import * as THREE from "three";

import type { WireStitch } from "@/lib/stitch-kinds";
import type { WasmSegment } from "@/lib/wasm";
import { buildYarnStrands, YARN_RADIUS, type Vec3 } from "@/lib/yarn-shape";

const YARN_COLOR = "#e8dcc8";
const FLAGGED_COLOR = "#e8543f";

// Engine axes: x/y are the sibling-ring plane (`geometry.rs`), z is
// stitch height — remapped here so height reads as "up" on screen
// (three.js: y-up). Same remap M4 used for the flat-line renderer.
function toThree(p: Vec3): THREE.Vector3 {
  return new THREE.Vector3(p.x, p.z, -p.y);
}

function YarnTubes({ segments, stitches }: { segments: WasmSegment[]; stitches: WireStitch[] }) {
  const tubes = useMemo(() => {
    const strands = buildYarnStrands(segments, stitches);
    return strands
      .filter((s) => s.points.length >= 2)
      .map((s) => {
        const points = s.points.map(toThree);
        const curve = new THREE.CatmullRomCurve3(points, false, "catmullrom", 0.5);
        // Scale sample count with point count so a long strand (many
        // stitches merged together, see yarn-shape.ts's strand-merging)
        // doesn't come out visibly faceted.
        const tubularSegments = Math.max(8, points.length * 3);
        const geometry = new THREE.TubeGeometry(curve, tubularSegments, YARN_RADIUS, 10, false);
        return { geometry, flagged: s.flagged };
      });
  }, [segments, stitches]);

  return (
    <>
      {tubes.map((t, i) => (
        <mesh key={i} geometry={t.geometry}>
          <meshStandardMaterial color={t.flagged ? FLAGGED_COLOR : YARN_COLOR} roughness={0.85} metalness={0.05} />
        </mesh>
      ))}
    </>
  );
}

/**
 * Renders the relaxed yarn path (from `crate::path::relaxed_yarn_segments`,
 * via the WASM bridge) as real, thick, per-stitch-shaped yarn — see
 * `lib/yarn-shape.ts` for the curve-generation logic and its documented
 * limits (M7: rendering-layer only, doesn't touch core/wasm's geometry).
 */
export default function YarnViewer({ segments, stitches }: { segments: WasmSegment[]; stitches: WireStitch[] }) {
  return (
    <Canvas
      camera={{ position: [4, 4, 6], fov: 45 }}
      className="h-full w-full"
      // preserveDrawingBuffer: not needed for on-screen rendering, but
      // without it, screenshot/capture tooling (CDP-based automated
      // screenshots, and eventually any in-app "export view" feature)
      // reads a stale/cleared buffer between frames — confirmed this
      // concretely while verifying M4 in a real browser: the page
      // rendered correctly the whole time, but automated screenshots
      // came back solid black until this was set.
      gl={{ preserveDrawingBuffer: true }}
    >
      <color attach="background" args={["#1a1a1a"]} />
      <ambientLight intensity={0.6} />
      <directionalLight position={[5, 8, 5]} intensity={1.1} />
      <directionalLight position={[-4, -2, -5]} intensity={0.3} />
      <YarnTubes segments={segments} stitches={stitches} />
      <OrbitControls makeDefault />
    </Canvas>
  );
}
