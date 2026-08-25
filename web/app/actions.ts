"use server";

import { prisma } from "@/lib/prisma";
import { generateSchemeSlug } from "@/lib/slug";
import { saveSchemeSchema, type SaveSchemeInput } from "@/lib/validation";

export type SaveSchemeResult = { ok: true; slug: string; name: string | null } | { ok: false; error: string };

// No accounts (M6 access-model decision): whoever holds `slug` can overwrite
// it, no ownership check. `slug` present and not found is treated as "start
// a new one" rather than an error — a stale/copied link shouldn't dead-end
// a save, and there's no owner identity to mismatch against anyway.
export async function saveScheme(input: SaveSchemeInput): Promise<SaveSchemeResult> {
  const parsed = saveSchemeSchema.safeParse(input);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? "invalid scheme" };
  }
  const { name, stitches, slug: existingSlug } = parsed.data;

  if (existingSlug) {
    const existing = await prisma.scheme.findUnique({ where: { slug: existingSlug } });
    if (existing) {
      const updated = await prisma.scheme.update({
        where: { slug: existingSlug },
        data: { name, stitches: stitches as object[] },
      });
      return { ok: true, slug: updated.slug, name: updated.name };
    }
  }

  let slug = generateSchemeSlug();
  // Collision is astronomically unlikely at 12 chars from a 32-symbol
  // alphabet, but retry a couple of times rather than trust that blindly —
  // same approach as when-we-meet's room slugs.
  for (let attempt = 0; attempt < 5; attempt++) {
    const clash = await prisma.scheme.findUnique({ where: { slug } });
    if (!clash) break;
    slug = generateSchemeSlug();
  }

  const created = await prisma.scheme.create({
    data: { slug, name, stitches: stitches as object[] },
  });
  return { ok: true, slug: created.slug, name: created.name };
}
