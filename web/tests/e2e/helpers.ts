import type { Page } from "@playwright/test";

/** The canvas is attached (and Playwright-"stable") before react-three-
 * fiber has actually finished its own WebGL/event-handler setup — a click
 * fired immediately after `goto` can land on a canvas that isn't hooked
 * up to raycasting yet and silently does nothing (confirmed: an earlier
 * heuristic here — waiting for `canvas.toDataURL()` to exceed a length
 * threshold — still flaked intermittently, since a freshly-cleared canvas
 * already serializes to more bytes than a small guessed threshold
 * accounts for). `YarnViewer.tsx`'s `Canvas` sets `data-r3f-ready` from
 * its own `onCreated` callback specifically so tests have a real signal
 * to wait on instead. */
async function waitForCanvasReady(page: Page) {
  await page.locator('canvas[data-r3f-ready="true"]').waitFor({ state: "visible" });
}

/** A click point that lands on visible geometry for a small scheme still
 * near the origin — verified by hand while building M8. Robust because a
 * `ch`/`mr` click ignores *what* it hits (any click places one, per
 * lib/tool-placement.ts), so this only needs to be "somewhere over the
 * yarn," not pixel-exact. */
export async function clickNearOrigin(page: Page) {
  await waitForCanvasReady(page);
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("canvas not found");
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.6);
}

export async function clickEmptyCorner(page: Page) {
  await waitForCanvasReady(page);
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("canvas not found");
  await page.mouse.click(box.x + box.width * 0.1, box.y + box.height * 0.1);
}

/**
 * Clicking a *specific* stitch's rendered mesh (as opposed to "anywhere
 * on the yarn," which `clickNearOrigin` covers) needs a screen point that
 * actually lands on it — and unlike `clickNearOrigin`'s use case, there's
 * no click-agnostic fallback here (a target-requiring tool ignores an
 * empty-space miss). Rather than hand-deriving the exact camera
 * projection for a given 3D point (real math, real risk of a subtly
 * wrong constant silently mis-clicking), this tries a small grid of
 * candidate points across the canvas and stops at the first one that
 * makes `isDone` true — `isDone` is checked from *outside* Playwright's
 * own auto-retry, so it must resolve quickly and reflect a real DOM read.
 */
export async function clickUntil(page: Page, isDone: () => Promise<boolean>, maxAttempts = 16): Promise<void> {
  await waitForCanvasReady(page);
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("canvas not found");

  const steps = Math.ceil(Math.sqrt(maxAttempts));
  let attempts = 0;
  for (let row = 1; row < steps && attempts < maxAttempts; row++) {
    for (let col = 1; col < steps && attempts < maxAttempts; col++) {
      if (await isDone()) return;
      attempts++;
      await page.mouse.click(box.x + (box.width * col) / steps, box.y + (box.height * row) / steps);
    }
  }
  if (!(await isDone())) {
    throw new Error(`clickUntil: condition still not met after ${attempts} grid clicks over the canvas`);
  }
}

/** Places `count` chains via the tool palette + render clicks — the M8
 * equivalent of the old "Add stitch" button, for tests that just need a
 * small deterministic scheme and aren't themselves testing placement
 * mechanics. Only selects the `ch` tool if it isn't already active —
 * clicking an *already*-active tool with nothing pending toggles it off
 * (see lib/tool-placement.ts's `selectTool`), so calling this more than
 * once in the same test must not blindly re-click it. */
export async function placeChains(page: Page, count: number) {
  const chTool = page.getByTestId("tool-ch");
  if ((await chTool.getAttribute("aria-pressed")) !== "true") {
    await chTool.click();
  }
  for (let i = 0; i < count; i++) {
    await clickNearOrigin(page);
  }
}
