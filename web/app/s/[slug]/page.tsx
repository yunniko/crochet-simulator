import { notFound } from "next/navigation";

import EditorApp from "@/app/EditorAppLoader";
import { prisma } from "@/lib/prisma";
import type { WireStitch } from "@/lib/stitch-kinds";

export default async function SavedSchemePage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const scheme = await prisma.scheme.findUnique({ where: { slug } });
  if (!scheme) notFound();

  return (
    <EditorApp
      initialStitches={scheme.stitches as unknown as WireStitch[]}
      initialSlug={scheme.slug}
      initialName={scheme.name}
    />
  );
}
