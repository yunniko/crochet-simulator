import type { WireScheme } from "@/lib/wasm";

export interface Preset {
  name: string;
  description: string;
  scheme: WireScheme;
}

export const PRESETS: Preset[] = [
  {
    name: "Flat circle (round 1)",
    description: "A tightened magic ring with 6 dc — the classic amigurumi opening round.",
    scheme: {
      stitches: [
        { kind: "mr", targets: [] },
        ...Array.from({ length: 6 }, () => ({ kind: "dc" as const, targets: [0] })),
      ],
    },
  },
  {
    name: "Overloaded ring (flagged)",
    description: "15 dc crammed into one tightened magic ring — well past comfortable capacity.",
    scheme: {
      stitches: [
        { kind: "mr", targets: [] },
        ...Array.from({ length: 15 }, () => ({ kind: "dc" as const, targets: [0] })),
      ],
    },
  },
  {
    name: "Shell (3 tr in one chain)",
    description: "A 3-tr shell into a single chain stitch — the granny-square norm, an ordinary multi-way share.",
    scheme: {
      stitches: [
        { kind: "ch", targets: [] },
        ...Array.from({ length: 3 }, () => ({ kind: "tr" as const, targets: [0] })),
      ],
    },
  },
  {
    name: "Freeform spike (non-row)",
    description:
      "A spike stitch: index 4 targets index 0, two stitches further back than its immediate predecessor — not \"the row below.\" Proves the model (and this editor) isn't row-locked, and validates cleanly.",
    scheme: {
      stitches: [
        { kind: "ch", targets: [] }, // 0
        { kind: "ch", targets: [] }, // 1
        { kind: "ch", targets: [] }, // 2
        { kind: "dc", targets: [2] }, // 3 — ordinary: targets the immediately preceding stitch
        { kind: "dc", targets: [0] }, // 4 — a spike: targets index 0, not index 3
      ],
    },
  },
];
