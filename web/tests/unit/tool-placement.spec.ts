import { describe, expect, it } from "vitest";

import {
  clickEmptySpace,
  clickStitch,
  INITIAL_PLACEMENT_STATE,
  isToolAvailable,
  selectTool,
  stitchRequiresTarget,
  type PlacementState,
} from "@/lib/tool-placement";
import type { StitchKind } from "@/lib/stitch-kinds";

// Most tests only care about *how many* stitches exist, not their kind —
// `dc` is an arbitrary non-foundation filler for those. Tests that care
// specifically about `start_ch`'s effect build their own kind arrays.
function kinds(count: number): StitchKind[] {
  return Array.from({ length: count }, () => "dc" as const);
}

describe("isToolAvailable", () => {
  it("allows mr only with zero stitches placed", () => {
    expect(isToolAvailable("mr", kinds(0))).toBe(true);
    expect(isToolAvailable("mr", kinds(1))).toBe(false);
    expect(isToolAvailable("mr", kinds(7))).toBe(false);
  });

  it("allows start_ch only with zero stitches placed", () => {
    expect(isToolAvailable("start_ch", kinds(0))).toBe(true);
    expect(isToolAvailable("start_ch", ["start_ch"])).toBe(false);
    expect(isToolAvailable("start_ch", kinds(3))).toBe(false);
  });

  it("blocks ch at the very start — start_ch replaces it there", () => {
    expect(isToolAvailable("ch", kinds(0))).toBe(false);
  });

  it("allows ch once any foundation stitch exists", () => {
    expect(isToolAvailable("ch", ["start_ch"])).toBe(true);
    expect(isToolAvailable("ch", ["mr"])).toBe(true);
    expect(isToolAvailable("ch", kinds(5))).toBe(true);
  });

  it("blocks every target-requiring kind until at least one stitch exists", () => {
    for (const kind of ["ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"] as const) {
      expect(isToolAvailable(kind, kinds(0))).toBe(false);
      expect(isToolAvailable(kind, kinds(1))).toBe(true);
    }
  });

  it("blocks target-requiring kinds while the only stitch so far is start_ch", () => {
    for (const kind of ["ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"] as const) {
      expect(isToolAvailable(kind, ["start_ch"])).toBe(false);
    }
  });

  it("unlocks target-requiring kinds as soon as a real chain exists alongside start_ch", () => {
    for (const kind of ["ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"] as const) {
      expect(isToolAvailable(kind, ["start_ch", "ch"])).toBe(true);
    }
  });

  it("unlocks target-requiring kinds immediately after mr, same as before", () => {
    for (const kind of ["ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"] as const) {
      expect(isToolAvailable(kind, ["mr"])).toBe(true);
    }
  });
});

describe("stitchRequiresTarget", () => {
  it("is false only for ch, mr, and start_ch", () => {
    expect(stitchRequiresTarget("ch")).toBe(false);
    expect(stitchRequiresTarget("mr")).toBe(false);
    expect(stitchRequiresTarget("start_ch")).toBe(false);
    for (const kind of ["ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"] as const) {
      expect(stitchRequiresTarget(kind)).toBe(true);
    }
  });
});

describe("selectTool", () => {
  it("does nothing for an unavailable tool", () => {
    const result = selectTool(INITIAL_PLACEMENT_STATE, "mr", kinds(3));
    expect(result).toEqual({ state: INITIAL_PLACEMENT_STATE, place: null });
  });

  it("activates an available tool without placing anything yet", () => {
    const result = selectTool(INITIAL_PLACEMENT_STATE, "start_ch", kinds(0));
    expect(result.place).toBeNull();
    expect(result.state).toEqual({ activeTool: "start_ch", pendingTargets: [] });
  });

  it("switching to a different tool abandons any pending target selection", () => {
    const midSelection = { activeTool: "dc" as const, pendingTargets: [2, 4] };
    const result = selectTool(midSelection, "tr", kinds(5));
    expect(result.place).toBeNull();
    expect(result.state).toEqual({ activeTool: "tr", pendingTargets: [] });
  });

  it("clicking the active start_ch/ch/mr tool again with no pending targets deselects it", () => {
    const result = selectTool({ activeTool: "start_ch", pendingTargets: [] }, "start_ch", kinds(0));
    expect(result).toEqual({ state: INITIAL_PLACEMENT_STATE, place: null });
  });

  it("clicking the active target-requiring tool again with pending targets confirms and places the stitch", () => {
    const state = { activeTool: "dc" as const, pendingTargets: [1, 3] };
    const result = selectTool(state, "dc", kinds(5));
    expect(result.place).toEqual({ kind: "dc", targets: [1, 3] });
    // Tool stays selected for the next placement — it's a tool, not a
    // one-shot action.
    expect(result.state).toEqual({ activeTool: "dc", pendingTargets: [] });
  });

  it("clicking the active target-requiring tool again with zero pending targets deselects instead of placing", () => {
    const state = { activeTool: "dc" as const, pendingTargets: [] };
    const result = selectTool(state, "dc", kinds(5));
    expect(result.place).toBeNull();
    expect(result.state).toEqual(INITIAL_PLACEMENT_STATE);
  });
});

describe("clickStitch, decrease mode off (the default)", () => {
  it("does nothing when no tool is active", () => {
    const result = clickStitch(INITIAL_PLACEMENT_STATE, 2, kinds(5), false);
    expect(result).toEqual({ state: INITIAL_PLACEMENT_STATE, place: null });
  });

  it("ch places immediately regardless of which stitch was clicked", () => {
    const state = { activeTool: "ch" as const, pendingTargets: [] };
    const result = clickStitch(state, 3, kinds(5), false);
    expect(result.place).toEqual({ kind: "ch", targets: [] });
    expect(result.state).toEqual(state);
  });

  it("a target-requiring tool places immediately with the single clicked target — no confirm click needed", () => {
    const state = { activeTool: "dc" as const, pendingTargets: [] };
    const result = clickStitch(state, 2, kinds(5), false);
    expect(result.place).toEqual({ kind: "dc", targets: [2] });
    // Tool stays active and pendingTargets stays empty — ready for
    // another single-click placement right away.
    expect(result.state).toEqual(state);
  });

  it("never accumulates pendingTargets, even across repeated clicks", () => {
    let state: PlacementState = { activeTool: "dc", pendingTargets: [] };
    const first = clickStitch(state, 3, kinds(6), false);
    expect(first.place).toEqual({ kind: "dc", targets: [3] });
    state = first.state;
    const second = clickStitch(state, 5, kinds(6), false);
    expect(second.place).toEqual({ kind: "dc", targets: [5] });
    expect(second.state.pendingTargets).toEqual([]);
  });
});

describe("clickStitch, decrease mode on", () => {
  it("a target-requiring tool accumulates the clicked index into pendingTargets instead of placing", () => {
    const state = { activeTool: "dc" as const, pendingTargets: [] };
    const result = clickStitch(state, 2, kinds(5), true);
    expect(result.place).toBeNull();
    expect(result.state).toEqual({ activeTool: "dc", pendingTargets: [2] });
  });

  it("clicking the same target again removes it (toggle), not a duplicate", () => {
    const state = { activeTool: "dc" as const, pendingTargets: [2, 4] };
    const result = clickStitch(state, 2, kinds(5), true);
    expect(result.place).toBeNull();
    expect(result.state).toEqual({ activeTool: "dc", pendingTargets: [4] });
  });

  it("multiple different clicks accumulate multiple pending targets, in click order", () => {
    let state: PlacementState = { activeTool: "dc", pendingTargets: [] };
    state = clickStitch(state, 3, kinds(6), true).state;
    state = clickStitch(state, 5, kinds(6), true).state;
    state = clickStitch(state, 1, kinds(6), true).state;
    expect(state.pendingTargets).toEqual([3, 5, 1]);
  });

  it("ch still places immediately — decrease mode only affects target-requiring kinds", () => {
    const state = { activeTool: "ch" as const, pendingTargets: [] };
    const result = clickStitch(state, 3, kinds(5), true);
    expect(result.place).toEqual({ kind: "ch", targets: [] });
  });
});

describe("clickEmptySpace", () => {
  it("does nothing when no tool is active", () => {
    expect(clickEmptySpace(INITIAL_PLACEMENT_STATE, kinds(0))).toEqual({
      state: INITIAL_PLACEMENT_STATE,
      place: null,
    });
  });

  it("ch places immediately", () => {
    const state = { activeTool: "ch" as const, pendingTargets: [] };
    const result = clickEmptySpace(state, kinds(3));
    expect(result.place).toEqual({ kind: "ch", targets: [] });
  });

  it("start_ch places immediately when the thread is still empty", () => {
    const state = { activeTool: "start_ch" as const, pendingTargets: [] };
    const result = clickEmptySpace(state, kinds(0));
    expect(result.place).toEqual({ kind: "start_ch", targets: [] });
  });

  it("mr places immediately when the thread is still empty", () => {
    const state = { activeTool: "mr" as const, pendingTargets: [] };
    const result = clickEmptySpace(state, kinds(0));
    expect(result.place).toEqual({ kind: "mr", targets: [] });
  });

  it("a target-requiring tool is a no-op on empty space — it has nothing to attach to", () => {
    const state = { activeTool: "dc" as const, pendingTargets: [1] };
    const result = clickEmptySpace(state, kinds(5));
    expect(result.place).toBeNull();
    expect(result.state).toEqual(state);
  });
});

describe("a full decrease-placement flow, end to end (decrease mode on)", () => {
  it("select dc, click two targets, confirm by re-clicking dc — matches the Owner's chosen design", () => {
    let state = INITIAL_PLACEMENT_STATE;
    state = selectTool(state, "dc", kinds(6)).state;
    state = clickStitch(state, 3, kinds(6), true).state;
    state = clickStitch(state, 5, kinds(6), true).state;
    const confirm = selectTool(state, "dc", kinds(6));
    expect(confirm.place).toEqual({ kind: "dc", targets: [3, 5] });
  });

  it("a single target still needs the confirm click while decrease mode is on", () => {
    let state = INITIAL_PLACEMENT_STATE;
    state = selectTool(state, "dc", kinds(6)).state;
    state = clickStitch(state, 4, kinds(6), true).state;
    const confirm = selectTool(state, "dc", kinds(6));
    expect(confirm.place).toEqual({ kind: "dc", targets: [4] });
  });
});

describe("a full single-target placement flow, end to end (decrease mode off, the default)", () => {
  it("select dc, click one target — placed immediately, no confirm click needed", () => {
    let state = INITIAL_PLACEMENT_STATE;
    state = selectTool(state, "dc", kinds(6)).state;
    const result = clickStitch(state, 4, kinds(6), false);
    expect(result.place).toEqual({ kind: "dc", targets: [4] });
  });
});

describe("a full starting-chain opening flow, end to end", () => {
  it("start_ch, then a real ch, unlocks post stitches in one step", () => {
    let placedKinds: StitchKind[] = [];
    // Place start_ch.
    let state = selectTool(INITIAL_PLACEMENT_STATE, "start_ch", placedKinds).state;
    let result = clickEmptySpace(state, placedKinds);
    expect(result.place).toEqual({ kind: "start_ch", targets: [] });
    placedKinds = [...placedKinds, "start_ch"];

    // Post stitches are still locked — only a real ch can follow.
    expect(isToolAvailable("dc", placedKinds)).toBe(false);
    expect(isToolAvailable("ch", placedKinds)).toBe(true);
    expect(isToolAvailable("start_ch", placedKinds)).toBe(false);
    expect(isToolAvailable("mr", placedKinds)).toBe(false);

    // Place the real chain.
    state = selectTool(state, "ch", placedKinds).state;
    result = clickEmptySpace(state, placedKinds);
    expect(result.place).toEqual({ kind: "ch", targets: [] });
    placedKinds = [...placedKinds, "ch"];

    // Now everything unlocks, same as the ordinary "≥1 stitch" rule.
    expect(isToolAvailable("dc", placedKinds)).toBe(true);
    expect(isToolAvailable("tr", placedKinds)).toBe(true);
  });
});
