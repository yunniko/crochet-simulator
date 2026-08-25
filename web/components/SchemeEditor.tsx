"use client";

import { useState } from "react";

import {
  CAPACITY_STYLES,
  LOOP_TARGETS,
  STITCH_KINDS,
  type CapacityStyle,
  type LoopTarget,
  type StitchKind,
  type WireStitch,
} from "@/lib/wasm";

interface SchemeEditorProps {
  stitches: WireStitch[];
  onAdd: (stitch: WireStitch) => void;
  onRemoveLast: () => void;
  onClear: () => void;
}

export default function SchemeEditor({ stitches, onAdd, onRemoveLast, onClear }: SchemeEditorProps) {
  const [kind, setKind] = useState<StitchKind>("dc");
  const [targets, setTargets] = useState<Set<number>>(new Set());
  const [loopTarget, setLoopTarget] = useState<LoopTarget>("Both");
  const [capacityOverride, setCapacityOverride] = useState<CapacityStyle | "">("");

  const toggleTarget = (index: number) => {
    setTargets((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const handleAdd = () => {
    onAdd({
      kind,
      targets: [...targets].sort((a, b) => a - b),
      ...(loopTarget !== "Both" ? { loop_target: loopTarget } : {}),
      ...(capacityOverride ? { capacity_override: capacityOverride } : {}),
    });
    // Next stitch commonly targets whatever was just added — carry that
    // forward instead of resetting to nothing, but let loop/capacity
    // settings reset since those are less often reused stitch-to-stitch.
    setTargets(new Set([stitches.length]));
    setLoopTarget("Both");
    setCapacityOverride("");
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-4 text-sm">
      <section>
        <h2 className="mb-2 font-medium text-zinc-300">Add stitch</h2>
        <div className="flex flex-col gap-2">
          <label className="flex items-center justify-between gap-2">
            <span className="text-zinc-400">Kind</span>
            <select
              value={kind}
              onChange={(e) => setKind(e.target.value as StitchKind)}
              className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-zinc-200"
            >
              {STITCH_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>

          <label className="flex items-center justify-between gap-2">
            <span className="text-zinc-400">Loop target</span>
            <select
              value={loopTarget}
              onChange={(e) => setLoopTarget(e.target.value as LoopTarget)}
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
              onChange={(e) => setCapacityOverride(e.target.value as CapacityStyle | "")}
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

          <div>
            <div className="mb-1 text-zinc-400">
              Targets ({targets.size} selected)
            </div>
            <div className="max-h-40 overflow-y-auto rounded border border-zinc-700 bg-zinc-800/50 p-1">
              {stitches.length === 0 && (
                <div className="px-2 py-1 text-zinc-500">No stitches yet — this one will be the foundation.</div>
              )}
              {stitches.map((s, i) => (
                <label
                  key={i}
                  className="flex cursor-pointer items-center gap-2 rounded px-2 py-1 hover:bg-zinc-700/50"
                >
                  <input type="checkbox" checked={targets.has(i)} onChange={() => toggleTarget(i)} />
                  <span className="text-zinc-300">
                    [{i}] {s.kind}
                  </span>
                </label>
              ))}
            </div>
          </div>

          <button
            onClick={handleAdd}
            className="mt-1 rounded bg-emerald-700 px-3 py-1.5 font-medium text-white hover:bg-emerald-600"
          >
            Add stitch [{stitches.length}]
          </button>
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
