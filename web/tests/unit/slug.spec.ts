import { describe, expect, it } from "vitest";

import { generateSchemeSlug } from "@/lib/slug";

describe("generateSchemeSlug", () => {
  it("generates a 12-character slug from the unambiguous alphabet only", () => {
    const slug = generateSchemeSlug();
    expect(slug).toHaveLength(12);
    expect(slug).toMatch(/^[23456789abcdefghjkmnpqrstuvwxyz]+$/);
  });

  it("never includes an ambiguous character (0, O, 1, I, l)", () => {
    for (let i = 0; i < 200; i++) {
      expect(generateSchemeSlug()).not.toMatch(/[0O1Il]/);
    }
  });

  it("generates distinct slugs across repeated calls", () => {
    const slugs = new Set(Array.from({ length: 50 }, () => generateSchemeSlug()));
    expect(slugs.size).toBe(50);
  });
});
