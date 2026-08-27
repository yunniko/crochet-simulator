"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

import { saveScheme } from "@/app/actions";
import SchemeEditor from "@/components/SchemeEditor";
import { PRESETS } from "@/lib/presets";
import type { WireStitch } from "@/lib/stitch-kinds";
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
  const [stitches, setStitches] = useState<WireStitch[]>(initialStitches ?? PRESETS[0].scheme.stitches);
  // `slug` tracks the *currently saved* link, if any — cleared implicitly
  // never; once a scheme has a slug, further saves overwrite it in place
  // (see actions.ts) rather than minting a new link on every edit+save.
  const [slug, setSlug] = useState<string | undefined>(initialSlug);
  const [name, setName] = useState<string>(initialName ?? "");

  return (
    <div className="flex h-dvh w-dvw flex-col bg-[#141414] text-zinc-200">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-zinc-800 px-4 py-3">
        <h1 className="text-sm font-medium tracking-wide text-zinc-400">
          crochet-sim <span className="text-zinc-600">— M6 editor</span>
        </h1>
        <div className="flex flex-wrap gap-2">
          {PRESETS.map((preset) => (
            <button
              key={preset.name}
              title={preset.description}
              onClick={() => {
                setStitches(preset.scheme.stitches);
                setSlug(undefined);
                setName("");
              }}
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
          <YarnViewer segments={result.segments} stitches={stitches} />
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
