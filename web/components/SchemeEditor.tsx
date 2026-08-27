"use client";

import { CAPACITY_STYLES, LOOP_TARGETS, STITCH_KINDS, type CapacityStyle, type LoopTarget, type StitchKind, type WireStitch } from "@/lib/wasm";
import { isToolAvailable, stitchRequiresTarget, type PlacementState } from "@/lib/tool-placement";

interface SchemeEditorProps {
  stitches: WireStitch[];
  placement: PlacementState;
  onSelectTool: (kind: StitchKind) => void;
  onRemoveLast: () => void;
  onClear: () => void;
  loopTarget: LoopTarget;
  onLoopTarget: (value: LoopTarget) => void;
  capacityOverride: CapacityStyle | "";
  onCapacityOverride: (value: CapacityStyle | "") => void;
}

export default function SchemeEditor({
  stitches,
  placement,
  onSelectTool,
  onRemoveLast,
  onClear,
  loopTarget,
  onLoopTarget,
  capacityOverride,
  onCapacityOverride,
}: SchemeEditorProps) {
  const { activeTool, pendingTargets } = placement;
  const activeNeedsTarget = activeTool !== null && stitchRequiresTarget(activeTool);

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-4 text-sm">
      <section>
        <h2 className="mb-1 font-medium text-zinc-300">Stitch tool</h2>
        <p className="mb-2 text-xs text-zinc-500">
          {stitches.length === 0
            ? "Pick chain or magic ring, then click the yarn to start."
            : activeNeedsTarget
              ? "Click the stitch(es) to insert into on the render, then click this tool again to place it."
              : "Pick a tool, then click the render to place it."}
        </p>
        <div className="grid grid-cols-3 gap-1.5" data-testid="tool-palette">
          {STITCH_KINDS.map((kind) => {
            const available = isToolAvailable(kind, stitches.length);
            const isActive = activeTool === kind;
            return (
              <button
                key={kind}
                data-testid={`tool-${kind}`}
                disabled={!available}
                aria-pressed={isActive}
                onClick={() => onSelectTool(kind)}
                title={
                  kind === "mr"
                    ? "Magic ring — only available as the very first stitch"
                    : kind === "ch"
                      ? "Chain — never needs a target"
                      : `${kind} — needs at least one existing stitch as a target`
                }
                className={`rounded px-2 py-2 text-xs font-medium uppercase transition-colors ${
                  isActive
                    ? "bg-sky-600 text-white"
                    : available
                      ? "bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
                      : "cursor-not-allowed bg-zinc-800/40 text-zinc-600"
                }`}
              >
                {kind}
              </button>
            );
          })}
        </div>
        {activeNeedsTarget && (
          <div data-testid="pending-targets" className="mt-2 text-xs text-sky-400">
            {pendingTargets.length === 0
              ? "No targets selected yet."
              : `Targets: [${pendingTargets.join(", ")}] — click "${activeTool}" again to place.`}
          </div>
        )}
      </section>

      <section>
        <h2 className="mb-2 font-medium text-zinc-300">Modifiers for the next stitch</h2>
        <div className="flex flex-col gap-2">
          <label className="flex items-center justify-between gap-2">
            <span className="text-zinc-400">Loop target</span>
            <select
              value={loopTarget}
              onChange={(e) => onLoopTarget(e.target.value as LoopTarget)}
              className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-zinc-200"
            >
              {LOOP_TARGETS.map((lt) => (
                <option key={lt} value={lt}>
                  {lt}
                </option>
              ))}
            </select>
          </label>

          <label className="flex items-center justify-between gap-2">
            <span className="text-zinc-400" title="Only matters if this stitch is later targeted, e.g. a magic ring">
              Capacity override
            </span>
            <select
              value={capacityOverride}
              onChange={(e) => onCapacityOverride(e.target.value as CapacityStyle | "")}
              className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-zinc-200"
            >
              <option value="">(default)</option>
              {CAPACITY_STYLES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </label>
        </div>
      </section>

      <section className="flex-1 min-h-0">
        <div className="mb-2 flex items-center justify-between">
          <h2 data-testid="stitch-count" className="font-medium text-zinc-300">
            Stitches ({stitches.length})
          </h2>
          <div className="flex gap-2">
            <button
              onClick={onRemoveLast}
              disabled={stitches.length === 0}
              className="rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-300 hover:bg-zinc-700 disabled:opacity-40"
            >
              Remove last
            </button>
            <button
              onClick={onClear}
              disabled={stitches.length === 0}
              className="rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-300 hover:bg-zinc-700 disabled:opacity-40"
            >
              Clear
            </button>
          </div>
        </div>
        <ol data-testid="stitch-list" className="space-y-0.5 font-mono text-xs text-zinc-400">
          {stitches.map((s, i) => (
            <li key={i} data-testid={`stitch-${i}`}>
              [{i}] {s.kind}
              {s.targets.length > 0 ? ` -> [${s.targets.join(", ")}]` : ""}
              {s.loop_target ? ` (${s.loop_target})` : ""}
              {s.capacity_override ? ` {${s.capacity_override}}` : ""}
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}
