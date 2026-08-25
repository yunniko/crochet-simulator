import { describe, expect, it } from "vitest";

import { saveSchemeSchema } from "@/lib/validation";

describe("saveSchemeSchema", () => {
  it("accepts a minimal valid scheme with no name or slug", () => {
    const result = saveSchemeSchema.safeParse({
      stitches: [{ kind: "mr", targets: [] }],
    });
    expect(result.success).toBe(true);
  });

  it("accepts every documented loop_target and capacity_override value", () => {
    const result = saveSchemeSchema.safeParse({
      stitches: [
        { kind: "ch", targets: [] },
        { kind: "dc", targets: [0], loop_target: "FrontOnly", capacity_override: "Elastic" },
      ],
    });
    expect(result.success).toBe(true);
  });

  it("rejects an unknown stitch kind", () => {
    const result = saveSchemeSchema.safeParse({
      stitches: [{ kind: "bobble", targets: [] }],
    });
    expect(result.success).toBe(false);
  });

  it("rejects an unknown loop_target", () => {
    const result = saveSchemeSchema.safeParse({
      stitches: [{ kind: "dc", targets: [], loop_target: "Sideways" }],
    });
    expect(result.success).toBe(false);
  });

  it("rejects a negative target index", () => {
    const result = saveSchemeSchema.safeParse({
      stitches: [{ kind: "dc", targets: [-1] }],
    });
    expect(result.success).toBe(false);
  });

  it("trims a blank name down to undefined rather than storing empty string", () => {
    const result = saveSchemeSchema.safeParse({
      name: "   ",
      stitches: [{ kind: "ch", targets: [] }],
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.name).toBeUndefined();
    }
  });

  it("accepts an empty stitch list (a cleared scheme is still a valid save)", () => {
    const result = saveSchemeSchema.safeParse({ stitches: [] });
    expect(result.success).toBe(true);
  });
});
