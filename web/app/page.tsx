"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

import { loadFlatCircleDemo, loadOverloadedDemo, type DemoResult } from "@/lib/wasm";

// react-three-fiber touches `window` on import — must never run during SSR.
const YarnViewer = dynamic(() => import("@/components/YarnViewer"), { ssr: false });

type DemoKind = "flat-circle" | "overloaded";

export default function Home() {
  const [kind, setKind] = useState<DemoKind>("flat-circle");

  return (
    <div className="flex h-dvh w-dvw flex-col bg-[#141414] text-zinc-200">
      <header className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
        <h1 className="text-sm font-medium tracking-wide text-zinc-400">
          crochet-sim <span className="text-zinc-600">— M4 viewer</span>
        </h1>
        <div className="flex gap-2">
          <DemoButton active={kind === "flat-circle"} onClick={() => setKind("flat-circle")}>
            Flat circle (valid)
          </DemoButton>
          <DemoButton active={kind === "overloaded"} onClick={() => setKind("overloaded")}>
            Overloaded ring (flagged)
          </DemoButton>
        </div>
      </header>

      {/* Keyed by `kind` so switching demos remounts fresh rather than
          needing to manually reset stale result/error state in an effect. */}
      <DemoPane key={kind} kind={kind} />
    </div>
  );
}

function DemoPane({ kind }: { kind: DemoKind }) {
  const [result, setResult] = useState<DemoResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let ignore = false;
    const loader = kind === "flat-circle" ? loadFlatCircleDemo : loadOverloadedDemo;
    loader()
      .then((data) => {
        if (!ignore) setResult(data);
      })
      .catch((e: unknown) => {
        if (!ignore) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      ignore = true;
    };
  }, [kind]);

  return (
    <main className="relative flex-1">
      {result && (
        <div className="absolute inset-0">
          <YarnViewer segments={result.segments} />
        </div>
      )}
      {!result && !error && (
        <div className="flex h-full items-center justify-center text-zinc-500">
          Computing scheme…
        </div>
      )}
      {error && (
        <div className="flex h-full items-center justify-center text-red-400">
          Error: {error}
        </div>
      )}

      {result && (
        <div className="absolute bottom-4 left-4 rounded-md border border-zinc-800 bg-[#1c1c1c]/90 px-4 py-3 text-sm shadow-lg backdrop-blur">
          <div className="mb-1 font-medium text-zinc-300">Model Statistics</div>
          <StatRow label="Stitches" value={result.stitch_count} />
          <StatRow
            label="Status"
            value={
              result.ok ? (
                <span className="text-emerald-400">OK</span>
              ) : (
                <span className="text-red-400">
                  Flagged ({result.violation_count} intersection
                  {result.violation_count === 1 ? "" : "s"})
                </span>
              )
            }
          />
        </div>
      )}
    </main>
  );
}

function DemoButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded px-3 py-1.5 text-xs font-medium transition-colors ${
        active
          ? "bg-zinc-200 text-zinc-900"
          : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
      }`}
    >
      {children}
    </button>
  );
}

function StatRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex justify-between gap-6 text-zinc-400">
      <span>{label}:</span>
      <span className="text-zinc-200">{value}</span>
    </div>
  );
}
