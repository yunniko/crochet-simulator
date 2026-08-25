import { z } from "zod";

import { CAPACITY_STYLES, LOOP_TARGETS, STITCH_KINDS } from "@/lib/stitch-kinds";

// Mirrors WireStitch/WireScheme (lib/wasm/index.ts, and wasm/src/lib.rs's
// own copy of the same shape) by hand — no shared codegen for these yet,
// see web/AGENTS.md. This is a *shape/type* check only (defence in depth
// against a request that bypasses the editor UI entirely, e.g. a raw POST),
// not the forward-reference semantic check `build_scheme_from_wire` already
// does — that one only matters when actually computing a scheme, and the
// editor already runs every stitch through `compute_scheme` before it's
// ever offered for saving, so re-deriving it here would just duplicate
// logic that lives correctly in Rust already.
const wireStitchSchema = z.object({
  kind: z.enum(STITCH_KINDS),
  targets: z.array(z.number().int().nonnegative()).max(64),
  loop_target: z.enum(LOOP_TARGETS).optional(),
  capacity_override: z.enum(CAPACITY_STYLES).optional(),
});

export const saveSchemeSchema = z.object({
  name: z
    .string()
    .trim()
    .max(120)
    .optional()
    .transform((v) => (v ? v : undefined)),
  stitches: z.array(wireStitchSchema).max(2000),
  // If present, updates that scheme in place rather than creating a new
  // one — the editor sends this once a scheme has been saved at least
  // once, so repeated saves during a session don't mint a new link each
  // time.
  slug: z.string().min(1).optional(),
});

export type SaveSchemeInput = z.infer<typeof saveSchemeSchema>;
