// M8: the click-to-place interaction model, as a pure state machine —
// framework-free so it's testable without React or three.js, and so the
// actual rules ("what can I click, what does it do") live in one place
// instead of being scattered across event handlers.
//
// The whole model is driven by three events: selecting a tool (clicking a
// palette button), clicking a stitch on the render, and clicking empty
// space on the render. Everything else (which tools are enabled, what a
// click does, when a stitch is actually placed) falls out of those three.

import type { StitchKind } from "@/lib/stitch-kinds";

// `ch`/`start_ch` never have a target (formed purely from the working
// loop — `start_ch` is a physical clone of `ch`, see core's own doc
// comment on it) and `mr` never has a target either (a single loop of
// yarn, not an insertion) — see docs/crochet-context.md §3/§4. Every
// other kind needs at least one.
export function stitchRequiresTarget(kind: StitchKind): boolean {
  return kind !== "ch" && kind !== "mr" && kind !== "start_ch";
}

// `mr` and `start_ch` can only start a thread — a magic ring is a
// foundation anchor (docs §5a), and `start_ch` is the chain-opening
// equivalent: the very first stitch, when the Owner opens with a chain
// rather than a magic ring. Unlike an ordinary `ch`, a lone `start_ch`
// isn't yet a real loop anything else can be worked into — post stitches
// stay locked out until an actual `ch` also exists (placed by extending
// the starting chain), at which point every kind unlocks in one step,
// same as the ordinary "at least one stitch exists" rule below. `ch`
// itself is no longer available at the very start (that's `start_ch`'s
// role now) but is otherwise always available once the foundation exists.
export function isToolAvailable(kind: StitchKind, placedKinds: readonly StitchKind[]): boolean {
  const count = placedKinds.length;
  if (kind === "mr") return count === 0;
  if (kind === "start_ch") return count === 0;
  if (kind === "ch") return count > 0;
  if (count === 0) return false;
  if (count === 1 && placedKinds[0] === "start_ch") return false;
  return true;
}

export interface PlacementState {
  activeTool: StitchKind | null;
  // Accumulated target indices for a target-requiring tool *in decrease
  // mode* — built up by clicking each target stitch in turn and confirmed
  // by clicking the active tool button again (see `selectTool` below).
  // Outside decrease mode a single target click places immediately (see
  // `clickStitch`), so this stays empty; always empty for `ch`/`mr` too,
  // which never have a target to accumulate.
  pendingTargets: number[];
}

export const INITIAL_PLACEMENT_STATE: PlacementState = { activeTool: null, pendingTargets: [] };

export interface PlacementInstruction {
  kind: StitchKind;
  targets: number[];
}

export interface PlacementResult {
  state: PlacementState;
  /** Set exactly when this event should actually place a stitch. */
  place: PlacementInstruction | null;
}

function unchanged(state: PlacementState): PlacementResult {
  return { state, place: null };
}

/**
 * Clicking a tool palette button. Selects it as the active tool — unless
 * it's *already* the active tool, in which case this is either "confirm
 * the pending multi-target selection" (if one exists) or "deselect the
 * tool" (if it doesn't, e.g. clicking `dc` twice with no targets clicked
 * in between just turns the tool off, rather than placing an invalid
 * zero-target stitch).
 */
export function selectTool(state: PlacementState, kind: StitchKind, placedKinds: readonly StitchKind[]): PlacementResult {
  if (!isToolAvailable(kind, placedKinds)) return unchanged(state);

  if (state.activeTool === kind) {
    if (stitchRequiresTarget(kind) && state.pendingTargets.length > 0) {
      // Confirm: place the stitch, tool stays selected (it's a tool, not
      // a one-shot action) for the next placement.
      return {
        state: { activeTool: kind, pendingTargets: [] },
        place: { kind, targets: state.pendingTargets },
      };
    }
    // Toggle off.
    return { state: INITIAL_PLACEMENT_STATE, place: null };
  }

  // Switching tools abandons any in-progress target selection — it isn't
  // meaningful for a different stitch kind, and the pending highlight
  // disappearing makes the abandonment visible, not a silent surprise.
  return unchanged({ activeTool: kind, pendingTargets: [] });
}

/**
 * Clicking an existing stitch's rendered shape.
 *
 * `decreaseMode` is an explicit opt-in, not the default: most placements
 * are single-target, so making every one of them a two-click "select
 * target, then click the tool again to confirm" affair would add a
 * needless click to the common case. With it off (the default), a single
 * click on a target immediately places the stitch. With it on, clicks
 * accumulate into a pending multi-target selection instead — confirmed
 * by clicking the active tool button again (see `selectTool`) — for
 * building a decrease.
 */
export function clickStitch(
  state: PlacementState,
  index: number,
  placedKinds: readonly StitchKind[],
  decreaseMode: boolean,
): PlacementResult {
  const tool = state.activeTool;
  if (!tool || !isToolAvailable(tool, placedKinds)) return unchanged(state);

  if (!stitchRequiresTarget(tool)) {
    // ch/mr/start_ch ignore *what* was clicked — any click places one,
    // per the model (they never have a target either way).
    return { state, place: { kind: tool, targets: [] } };
  }

  if (!decreaseMode) {
    return { state, place: { kind: tool, targets: [index] } };
  }

  // Toggle the clicked stitch in/out of the pending target set, rather
  // than only ever adding — clicking a target you already picked by
  // mistake should un-pick it, not duplicate it.
  const pendingTargets = state.pendingTargets.includes(index)
    ? state.pendingTargets.filter((i) => i !== index)
    : [...state.pendingTargets, index];
  return unchanged({ activeTool: tool, pendingTargets });
}

/** Clicking empty space on the render (`Canvas`'s `onPointerMissed`). */
export function clickEmptySpace(state: PlacementState, placedKinds: readonly StitchKind[]): PlacementResult {
  const tool = state.activeTool;
  if (!tool || !isToolAvailable(tool, placedKinds)) return unchanged(state);
  if (stitchRequiresTarget(tool)) {
    // Target-requiring kinds can only be placed by clicking their target
    // directly — an empty-space click has nothing to attach them to.
    return unchanged(state);
  }
  return { state, place: { kind: tool, targets: [] } };
}
