"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

import { saveScheme } from "@/app/actions";
import SchemeEditor from "@/components/SchemeEditor";
import { PRESETS } from "@/lib/presets";
import type { CapacityStyle, LoopTarget, StitchKind, WireStitch } from "@/lib/stitch-kinds";
import {
  clickEmptySpace,
  clickStitch,
  INITIAL_PLACEMENT_STATE,
  isToolAvailable,
  selectTool,
  stitchRequiresTarget,
  type PlacementResult,
  type PlacementState,
} from "@/lib/tool-placement";
import { computeScheme, type ComputeResult } from "@/lib/wasm";

// react-three-fiber touches `window` on import — must never run during SSR.
const YarnViewer = dynamic(() => import("@/components/YarnViewer"), { ssr: false });

interface EditorAppProps {
  /** Set when loaded from a saved scheme's `/s/[slug]` page; absent on `/`. */
  initialStitches?: WireStitch[];
  initialSlug?: string;
  initialName?: string | null;
}

export default function EditorApp({ initialStitches, initialSlug, initialName }: EditorAppProps) {
  // Empty by default (M8): the app starts with just the undecorated
  // starting yarn stub (see YarnViewer), ready to build from scratch by
  // clicking tools/render — not a preloaded example. Presets remain
  // available as alternate starting points via the header buttons.
  const [stitches, setStitches] = useState<WireStitch[]>(initialStitches ?? []);
  const [placement, setPlacement] = useState<PlacementState>(INITIAL_PLACEMENT_STATE);
  // Off by default: a single click on a target immediately places the
  // stitch (the common case). Decrease mode is an explicit opt-in — click
  // each target in turn, click the active tool again to confirm — so it
  // doesn't add an extra confirm click to every ordinary placement.
  const [decreaseMode, setDecreaseMode] = useState(false);
  const [loopTarget, setLoopTarget] = useState<LoopTarget>("Both");
  const [capacityOverride, setCapacityOverride] = useState<CapacityStyle | "">("");
  // `slug` tracks the *currently saved* link, if any — cleared implicitly
  // never; once a scheme has a slug, further saves overwrite it in place
  // (see actions.ts) rather than minting a new link on every edit+save.
  const [slug, setSlug] = useState<string | undefined>(initialSlug);
  const [name, setName] = useState<string>(initialName ?? "");

  // A single place where "a placement decision happened" (tool select,
  // stitch click, empty-space click — see lib/tool-placement.ts) turns
  // into an actual WireStitch, so the three call sites below don't
  // duplicate this. Also guards against the newly-active tool becoming
  // unavailable as a direct result of the placement (e.g. `mr` right
  // after it places the very first stitch) — without this the palette
  // would show a tool as both "active" and "disabled" at once.
  const applyPlacementResult = (result: PlacementResult) => {
    if (!result.place) {
      setPlacement(result.state);
      return;
    }
    const { kind, targets } = result.place;
    const newStitch: WireStitch = {
      kind,
      targets,
      ...(stitchRequiresTarget(kind) && loopTarget !== "Both" ? { loop_target: loopTarget } : {}),
      ...(capacityOverride ? { capacity_override: capacityOverride } : {}),
    };
    // Uses the `stitches` this render closed over, not a functional
    // updater — each call is one discrete synchronous click event, never
    // concurrent with another, so this is safe and avoids calling
    // setPlacement from inside setStitches's updater (which would violate
    // the updater's purity contract).
    const nextStitches = [...stitches, newStitch];
    setStitches(nextStitches);
    setPlacement(isToolAvailable(result.state.activeTool!, nextStitches.length) ? result.state : INITIAL_PLACEMENT_STATE);
  };

  const loadScheme = (newStitches: WireStitch[]) => {
    setStitches(newStitches);
    setPlacement(INITIAL_PLACEMENT_STATE);
    setSlug(undefined);
    setName("");
  };

  return (
    <div className="flex h-dvh w-dvw flex-col bg-[#141414] text-zinc-200">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-zinc-800 px-4 py-3">
        <h1 className="text-sm font-medium tracking-wide text-zinc-400">
          crochet-sim <span className="text-zinc-600">— M8 editor</span>
        </h1>
        <div className="flex flex-wrap gap-2">
          {PRESETS.map((preset) => (
            <button
              key={preset.name}
              title={preset.description}
              onClick={() => loadScheme(preset.scheme.stitches)}
              className="rounded bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-300 transition-colors hover:bg-zinc-700"
            >
              {preset.name}
            </button>
          ))}
        </div>
        <SaveControls stitches={stitches} slug={slug} name={name} onName={setName} onSaved={setSlug} />
      </header>

      <div className="flex flex-1 overflow-hidden">
        <aside className="w-80 shrink-0 border-r border-zinc-800">
          <SchemeEditor
            stitches={stitches}
            placement={placement}
            onSelectTool={(kind: StitchKind) => applyPlacementResult(selectTool(placement, kind, stitches.length))}
            decreaseMode={decreaseMode}
            onDecreaseMode={(value: boolean) => {
              setDecreaseMode(value);
              // Turning it off mid-selection abandons any in-progress
              // multi-target pick — same reasoning as switching tools:
              // it isn't meaningful once single-click-to-place is back,
              // and the highlight disappearing makes that visible.
              setPlacement((p) => ({ ...p, pendingTargets: [] }));
            }}
            onRemoveLast={() => {
              setStitches((prev) => prev.slice(0, -1));
              // The removed stitch's index may no longer exist — drop any
              // pending target selection rather than leave it pointing at
              // a stitch that's gone.
              setPlacement((p) => ({ ...p, pendingTargets: [] }));
            }}
            onClear={() => {
              setStitches([]);
              setPlacement(INITIAL_PLACEMENT_STATE);
            }}
            loopTarget={loopTarget}
            onLoopTarget={setLoopTarget}
            capacityOverride={capacityOverride}
            onCapacityOverride={setCapacityOverride}
          />
        </aside>

        <ComputePane
          stitches={stitches}
          placement={placement}
          onStitchClick={(index: number) =>
            applyPlacementResult(clickStitch(placement, index, stitches.length, decreaseMode))
          }
          onEmptySpaceClick={() => applyPlacementResult(clickEmptySpace(placement, stitches.length))}
        />
      </div>
    </div>
  );
}

function SaveControls({
  stitches,
  slug,
  name,
  onName,
  onSaved,
}: {
  stitches: WireStitch[];
  slug: string | undefined;
  name: string;
  onName: (name: string) => void;
  onSaved: (slug: string) => void;
}) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Reading `window` directly during render is safe here specifically
  // because EditorApp itself is loaded via `dynamic(..., { ssr: false })`
  // (see page.tsx / s/[slug]/page.tsx) — it never has an SSR pass to
  // mismatch against. Don't reuse this pattern in a component that *is*
  // server-rendered.
  const shareUrl = slug ? `${window.location.origin}/s/${slug}` : null;

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setCopied(false);
    try {
      const result = await saveScheme({ stitches, name: name || undefined, slug });
      if (result.ok) {
        onSaved(result.slug);
        // Reflects the saved link in the address bar without a full
        // navigation/reload — the editor state (and the loaded WASM
        // module) stays exactly as it is.
        window.history.replaceState(null, "", `/s/${result.slug}`);
      } else {
        setError(result.error);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleCopy = async () => {
    if (!shareUrl) return;
    try {
      await navigator.clipboard.writeText(shareUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard access can fail (permissions, insecure context) — the
      // link is still shown as selectable text, so this is a convenience,
      // not the only way to get it.
    }
  };

  return (
    <div className="flex items-center gap-2">
      <input
        type="text"
        placeholder="Scheme name (optional)"
        value={name}
        onChange={(e) => onName(e.target.value)}
        data-testid="scheme-name-input"
        className="w-40 rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs text-zinc-200 placeholder:text-zinc-500"
      />
      <button
        onClick={handleSave}
        disabled={saving || stitches.length === 0}
        data-testid="save-button"
        className="rounded bg-emerald-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-600 disabled:opacity-40"
      >
        {saving ? "Saving…" : slug ? "Save changes" : "Save"}
      </button>
      {error && <span className="text-xs text-red-400">{error}</span>}
      {shareUrl && !error && (
        <button
          onClick={handleCopy}
          data-testid="share-link"
          title={shareUrl}
          className="max-w-48 truncate rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-400 hover:bg-zinc-700"
        >
          {copied ? "Copied!" : shareUrl}
        </button>
      )}
    </div>
  );
}

function ComputePane({
  stitches,
  placement,
  onStitchClick,
  onEmptySpaceClick,
}: {
  stitches: WireStitch[];
  placement: PlacementState;
  onStitchClick: (index: number) => void;
  onEmptySpaceClick: () => void;
}) {
  const [result, setResult] = useState<ComputeResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Deliberately doesn't reset result/error to null when stitches is
    // empty or while a new computation is in flight — the viewer below
    // gates its own empty-state stub on `stitches.length` directly, and
    // keeps showing the previous scheme's tubes while the next
    // computation runs (rather than flashing/disappearing on every single
    // click) which matters more now than it did pre-M8: every click that
    // places a stitch triggers a recompute, so this runs constantly
    // during normal use, not just on the rare manual edit.
    if (stitches.length === 0) return;
    let ignore = false;
    computeScheme({ stitches })
      .then((data) => {
        if (!ignore) {
          setResult(data);
          setError(null);
        }
      })
      .catch((e: unknown) => {
        if (!ignore) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      ignore = true;
    };
  }, [stitches]);

  return (
    <main className="relative flex-1">
      <div className="absolute inset-0">
        <YarnViewer
          segments={result?.segments ?? []}
          stitches={stitches}
          pendingTargets={placement.pendingTargets}
          hasActiveTool={placement.activeTool !== null}
          onStitchClick={onStitchClick}
          onEmptySpaceClick={onEmptySpaceClick}
        />
      </div>

      {stitches.length > 0 && !result && !error && (
        <div className="pointer-events-none absolute inset-x-0 top-4 text-center text-xs text-zinc-500">
          Computing scheme…
        </div>
      )}
      {error && (
        <div className="pointer-events-none absolute inset-x-0 top-4 px-8 text-center text-xs text-red-400">
          Error: {error}
        </div>
      )}

      {/* `stitches.length > 0` (not just `result`) — otherwise a Clear
          leaves the last scheme's stats visibly on screen, including a
          stale "Flagged" reading for a now-empty scheme that has nothing
          to be flagged. `result` itself is deliberately left stale during
          an in-flight recompute of a *non-empty* scheme, per the effect's
          own comment above — this only hides it for the genuinely-empty
          case. */}
      {stitches.length > 0 && result && (
        <div className="pointer-events-none absolute bottom-4 left-4 rounded-md border border-zinc-800 bg-[#1c1c1c]/90 px-4 py-3 text-sm shadow-lg backdrop-blur">
          <div className="mb-1 font-medium text-zinc-300">Model Statistics</div>
          <StatRow label="Stitches" value={result.stitch_count} testId="stat-stitches" />
          <StatRow
            label="Status"
            testId="stat-status"
            value={
              result.ok ? (
                <span className="text-emerald-400">OK</span>
              ) : (
                <span className="text-red-400">
                  Flagged ({result.violation_count} intersection{result.violation_count === 1 ? "" : "s"})
                </span>
              )
            }
          />
        </div>
      )}
    </main>
  );
}

function StatRow({ label, value, testId }: { label: string; value: React.ReactNode; testId: string }) {
  return (
    <div className="flex justify-between gap-6 text-zinc-400">
      <span>{label}:</span>
      <span data-testid={testId} className="text-zinc-200">
        {value}
      </span>
    </div>
  );
}
