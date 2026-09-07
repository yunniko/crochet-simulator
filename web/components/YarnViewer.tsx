"use client";

import { OrbitControls } from "@react-three/drei";
import { Canvas, useFrame, useThree, type ThreeEvent } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import type { OrbitControls as OrbitControlsImpl } from "three-stdlib";

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
  pendingTargets: number[];
  hasActiveTool: boolean;
  onStitchClick: (index: number) => void;
}

function YarnTubes({ segments, pendingTargets, hasActiveTool, onStitchClick }: YarnTubesProps) {
  const strands = useMemo(() => buildYarnStrands(segments), [segments]);

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

/**
 * Frames the camera to whatever geometry actually exists, instead of a
 * fixed position/distance tuned for one small demo scheme. Needed once the
 * default starting scheme became a real, much larger rope (`lib/starting-
 * rope.ts`, 2026-08-30) — the old fixed `camera={{ position: [4, 4, 6] }}`
 * left most of it outside the view frustum entirely. This computes a real
 * bounding sphere from the actual computed segment points (not a guessed
 * size) and positions the camera to fit it, keeping the same viewing angle
 * the fixed camera used. Deliberately only re-fits on the transition from
 * empty to non-empty (on mount with a real starting scheme, and again
 * after a Clear + rebuild) — not on every single stitch placement, which
 * would otherwise yank the camera away from wherever the Owner has
 * manually orbited to while building. Driven by `useFrame` (r3f's own
 * render loop) rather than `useEffect`, since the fit needs to run as part
 * of an actual rendered frame, not just a React commit.
 */
function CameraFit({ segments, isEmpty, controlsRef }: { segments: WasmSegment[]; isEmpty: boolean; controlsRef: React.RefObject<OrbitControlsImpl | null> }) {
  const { camera } = useThree();
  const framedSinceEmptyRef = useRef(false);

  useFrame(() => {
    if (isEmpty) {
      framedSinceEmptyRef.current = false;
      return;
    }
    if (framedSinceEmptyRef.current || segments.length === 0) return;
    framedSinceEmptyRef.current = true;

    const box = new THREE.Box3();
    for (const seg of segments) {
      box.expandByPoint(toThree(seg.start));
      box.expandByPoint(toThree(seg.end));
    }
    const center = box.getCenter(new THREE.Vector3());
    const radius = Math.max(box.getSize(new THREE.Vector3()).length() / 2, 1);

    const perspective = camera as THREE.PerspectiveCamera;
    const fovRadians = (perspective.fov * Math.PI) / 180;
    // 1.25x margin so the geometry doesn't touch the viewport edges.
    const distance = (radius / Math.sin(fovRadians / 2)) * 1.25;

    // Same viewing angle the old fixed camera used ([4, 4, 6]), just
    // scaled to a distance that actually fits the real content.
    const direction = new THREE.Vector3(4, 4, 6).normalize();
    camera.position.copy(center.clone().addScaledVector(direction, distance));
    camera.lookAt(center);
    perspective.updateProjectionMatrix();

    if (controlsRef.current) {
      controlsRef.current.target.copy(center);
      controlsRef.current.update();
    }
  });

  return null;
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
 * `Strand.stitchIndex`). A genuinely empty scheme (`stitches.length === 0`,
 * reached via "Clear" — the app no longer starts empty by default, see
 * `lib/starting-rope.ts`) renders nothing at all: no decorative placeholder
 * (per the Owner's standing "nothing decorative" instruction), just an
 * empty scene an `onPointerMissed` click still works against. The
 * `stitches` prop still matters here even though it's not itself rendered
 * — it's what tells this component to stop showing `segments` at all,
 * rather than a stale render of whatever scheme was computed most recently
 * (`ComputePane` deliberately leaves `result` stale during Clear, see its
 * own comment).
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
  const controlsRef = useRef<OrbitControlsImpl | null>(null);
  return (
    <Canvas
      camera={{ position: [4, 4, 6], fov: 45 }}
      frameloop="always"
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
      {isEmpty ? null : (
        <YarnTubes
          segments={segments}
          pendingTargets={pendingTargets}
          hasActiveTool={hasActiveTool}
          onStitchClick={onStitchClick}
        />
      )}
      <CameraFit segments={segments} isEmpty={isEmpty} controlsRef={controlsRef} />
      <OrbitControls ref={controlsRef} makeDefault />
    </Canvas>
  );
}
