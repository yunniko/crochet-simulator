"use client";

import { OrbitControls } from "@react-three/drei";
import { Canvas } from "@react-three/fiber";
import { useMemo } from "react";
import * as THREE from "three";

import type { WasmSegment } from "@/lib/wasm";

const YARN_COLOR = "#e8dcc8";
const FLAGGED_COLOR = "#e8543f";

function YarnLines({ segments }: { segments: WasmSegment[] }) {
  // Two LineSegments meshes (ordinary + flagged) rather than one line per
  // segment — a single draw call per colour is plenty for a design-tool-
  // sized scheme and keeps this simple for M4's minimal viewer.
  const { ordinary, flagged } = useMemo(() => {
    const ordinaryPoints: number[] = [];
    const flaggedPoints: number[] = [];
    for (const s of segments) {
      const target = s.flagged ? flaggedPoints : ordinaryPoints;
      target.push(s.start.x, s.start.z, -s.start.y);
      target.push(s.end.x, s.end.z, -s.end.y);
    }
    const toGeometry = (points: number[]) => {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.Float32BufferAttribute(points, 3));
      return geometry;
    };
    return {
      ordinary: toGeometry(ordinaryPoints),
      flagged: toGeometry(flaggedPoints),
    };
  }, [segments]);

  return (
    <>
      <lineSegments geometry={ordinary}>
        <lineBasicMaterial color={YARN_COLOR} linewidth={2} />
      </lineSegments>
      <lineSegments geometry={flagged}>
        <lineBasicMaterial color={FLAGGED_COLOR} linewidth={3} />
      </lineSegments>
    </>
  );
}

/**
 * Renders a relaxed yarn path (from `crate::path::relaxed_yarn_segments`,
 * via the WASM bridge). Engine axes: x/y are the sibling-ring plane
 * (`geometry.rs`), z is stitch height — remapped here so height reads as
 * "up" on screen (three.js: y-up).
 */
export default function YarnViewer({ segments }: { segments: WasmSegment[] }) {
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
      <ambientLight intensity={0.7} />
      <directionalLight position={[5, 8, 5]} intensity={0.8} />
      <YarnLines segments={segments} />
      <OrbitControls makeDefault />
    </Canvas>
  );
}
