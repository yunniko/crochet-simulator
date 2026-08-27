"use client";

import { OrbitControls } from "@react-three/drei";
import { Canvas, type ThreeEvent } from "@react-three/fiber";
import { useMemo } from "react";
import * as THREE from "three";

import type { WireStitch } from "@/lib/stitch-kinds";
import type { WasmSegment } from "@/lib/wasm";
import { buildYarnStrands, YARN_RADIUS, type Vec3 } from "@/lib/yarn-shape";

const YARN_COLOR = "#e8dcc8";
const FLAGGED_COLOR = "#e8543f";
const PENDING_COLOR = "#5ea8e8";

// Engine axes: x/y are the sibling-ring plane (`geometry.rs`), z is
// stitch height — remapped here so height reads as "up" on screen
// (three.js: y-up). Same remap M4 used for the flat-line renderer.
function toThree(p: Vec3): THREE.Vector3 {
  return new THREE.Vector3(p.x, p.z, -p.y);
}

function tubeFromPoints(points: Vec3[]): THREE.TubeGeometry | null {
  if (points.length < 2) return null;
  const threePoints = points.map(toThree);
  const curve = new THREE.CatmullRomCurve3(threePoints, false, "catmullrom", 0.5);
  // Scale sample count with point count so a longer curve (a taller
  // stitch's wiggle) doesn't come out visibly faceted.
  const tubularSegments = Math.max(8, threePoints.length * 3);
  return new THREE.TubeGeometry(curve, tubularSegments, YARN_RADIUS, 10, false);
}

interface YarnTubesProps {
  segments: WasmSegment[];
  stitches: WireStitch[];
  pendingTargets: number[];
  hasActiveTool: boolean;
  onStitchClick: (index: number) => void;
}

function YarnTubes({ segments, stitches, pendingTargets, hasActiveTool, onStitchClick }: YarnTubesProps) {
  const strands = useMemo(() => buildYarnStrands(segments, stitches), [segments, stitches]);

  return (
    <>
      {strands.map((s, i) => {
        const geometry = tubeFromPoints(s.points);
        if (!geometry) return null;
        const isClickTarget = s.stitchIndex !== null;
        const isPending = s.stitchIndex !== null && pendingTargets.includes(s.stitchIndex);
        const color = isPending ? PENDING_COLOR : s.flagged ? FLAGGED_COLOR : YARN_COLOR;
        return (
          <mesh
            key={i}
            geometry={geometry}
            onClick={
              isClickTarget && hasActiveTool
                ? (event: ThreeEvent<MouseEvent>) => {
                    event.stopPropagation();
                    onStitchClick(s.stitchIndex as number);
                  }
                : undefined
            }
          >
            <meshStandardMaterial color={color} roughness={0.85} metalness={0.05} />
          </mesh>
        );
      })}
    </>
  );
}

/** The undecorated starting piece of yarn shown before any stitch exists — clicking it is exactly an empty-space click (`Canvas`'s `onPointerMissed` handles it), it's just something to see and aim at. */
function StartingYarnStub() {
  const geometry = useMemo(
    () => tubeFromPoints([{ x: 0, y: 0, z: -1.2 }, { x: 0, y: 0, z: 0 }]),
    [],
  );
  if (!geometry) return null;
  return (
    <mesh geometry={geometry}>
      <meshStandardMaterial color={YARN_COLOR} roughness={0.85} metalness={0.05} />
    </mesh>
  );
}

export interface YarnViewerProps {
  segments: WasmSegment[];
  stitches: WireStitch[];
  /** Indices currently selected as targets for the tool in hand, mid-placement — see `lib/tool-placement.ts`. */
  pendingTargets: number[];
  /** Whether a placement tool is currently selected — clicks only do anything while one is. */
  hasActiveTool: boolean;
  onStitchClick: (index: number) => void;
  onEmptySpaceClick: () => void;
}

/**
 * Renders the relaxed yarn path (from `crate::path::relaxed_yarn_segments`,
 * via the WASM bridge) as real, thick, per-stitch-shaped yarn — see
 * `lib/yarn-shape.ts` for the curve-generation logic and its documented
 * limits. M8: also the click surface for the direct-manipulation editor —
 * each stitch is its own clickable mesh (not merged with neighbours, see
 * `Strand.stitchIndex`), and an empty scheme shows a plain starting stub
 * instead of nothing, since the render is how building a scheme starts
 * now, not just how it's displayed afterward.
 */
export default function YarnViewer({
  segments,
  stitches,
  pendingTargets,
  hasActiveTool,
  onStitchClick,
  onEmptySpaceClick,
}: YarnViewerProps) {
  const isEmpty = stitches.length === 0;
  return (
    <Canvas
      camera={{ position: [4, 4, 6], fov: 45 }}
      className="h-full w-full"
      style={{ cursor: hasActiveTool ? "pointer" : "auto" }}
      // preserveDrawingBuffer: not needed for on-screen rendering, but
      // without it, screenshot/capture tooling (CDP-based automated
      // screenshots, and eventually any in-app "export view" feature)
      // reads a stale/cleared buffer between frames — confirmed this
      // concretely while verifying M4 in a real browser: the page
      // rendered correctly the whole time, but automated screenshots
      // came back solid black until this was set.
      gl={{ preserveDrawingBuffer: true }}
      // Fires when a pointer event doesn't hit any mesh — exactly an
      // "empty space" click, per the M8 interaction model.
      onPointerMissed={() => hasActiveTool && onEmptySpaceClick()}
      // A real readiness signal for e2e tests (tests/e2e/helpers.ts):
      // the canvas element is attached (and Playwright-"stable") well
      // before r3f's own renderer/raycasting is actually wired up to it —
      // clicking too early silently does nothing. `onCreated` only fires
      // once that setup has genuinely finished.
      onCreated={(state) => state.gl.domElement.setAttribute("data-r3f-ready", "true")}
    >
      <color attach="background" args={["#1a1a1a"]} />
      <ambientLight intensity={0.6} />
      <directionalLight position={[5, 8, 5]} intensity={1.1} />
      <directionalLight position={[-4, -2, -5]} intensity={0.3} />
      {isEmpty ? (
        <StartingYarnStub />
      ) : (
        <YarnTubes
          segments={segments}
          stitches={stitches}
          pendingTargets={pendingTargets}
          hasActiveTool={hasActiveTool}
          onStitchClick={onStitchClick}
        />
      )}
      <OrbitControls makeDefault />
    </Canvas>
  );
}
