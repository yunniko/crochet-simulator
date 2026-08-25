"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

import SchemeEditor from "@/components/SchemeEditor";
import { PRESETS } from "@/lib/presets";
import { computeScheme, type ComputeResult, type WireStitch } from "@/lib/wasm";

// react-three-fiber touches `window` on import — must never run during SSR.
const YarnViewer = dynamic(() => import("@/components/YarnViewer"), { ssr: false });

export default function Home() {
  const [stitches, setStitches] = useState<WireStitch[]>(PRESETS[0].scheme.stitches);

  return (
    <div className="flex h-dvh w-dvw flex-col bg-[#141414] text-zinc-200">
      <header className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
        <h1 className="text-sm font-medium tracking-wide text-zinc-400">
          crochet-sim <span className="text-zinc-600">— M5 editor</span>
        </h1>
        <div className="flex gap-2">
          {PRESETS.map((preset) => (
            <button
              key={preset.name}
              title={preset.description}
              onClick={() => setStitches(preset.scheme.stitches)}
              className="rounded bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-300 transition-colors hover:bg-zinc-700"
            >
              {preset.name}
            </button>
          ))}
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        <aside className="w-80 shrink-0 border-r border-zinc-800">
          <SchemeEditor
            stitches={stitches}
            onAdd={(s) => setStitches((prev) => [...prev, s])}
            onRemoveLast={() => setStitches((prev) => prev.slice(0, -1))}
            onClear={() => setStitches([])}
          />
        </aside>

        <ComputePane stitches={stitches} />
      </div>
    </div>
  );
}

function ComputePane({ stitches }: { stitches: WireStitch[] }) {
  const [result, setResult] = useState<ComputeResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Deliberately doesn't reset result/error to null when stitches is
    // empty or while a new computation is in flight — the render below
    // gates on `stitches.length` directly for the empty state, and briefly
    // showing the previous scheme while the next one computes (rather
    // than flashing to a loading state on every edit) is the better UX
    // here anyway.
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
      {stitches.length > 0 && result && (
        <div className="absolute inset-0">
          <YarnViewer segments={result.segments} />
        </div>
      )}
      {stitches.length === 0 && (
        <div className="flex h-full items-center justify-center text-zinc-500">
          Add a stitch, or load a preset, to get started.
        </div>
      )}
      {stitches.length > 0 && !result && !error && (
        <div className="flex h-full items-center justify-center text-zinc-500">Computing scheme…</div>
      )}
      {error && (
        <div className="flex h-full items-center justify-center px-8 text-center text-red-400">Error: {error}</div>
      )}

      {result && (
        <div className="absolute bottom-4 left-4 rounded-md border border-zinc-800 bg-[#1c1c1c]/90 px-4 py-3 text-sm shadow-lg backdrop-blur">
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
