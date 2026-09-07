import { expect, test } from "@playwright/test";

import { clickEmptyCorner, clickNearOrigin, clickUntil } from "./helpers";

// M8: the editor is a direct-manipulation, click-on-the-render tool
// palette, not a form. Since 2026-08-30 (G-002) the app starts with a
// real, simulated long chain already placed (STARTING_ROPE — see
// lib/starting-rope.ts) rather than an empty canvas or a preset picker —
// presets were removed entirely. "Clear" still resets to a true empty
// scheme (start_ch/mr only) for building entirely from scratch. Asserts
// via data-testid hooks on the tool palette / pending-target hint / stats
// — not on canvas pixels (see HANDOVER.md's note on the screenshot-timing
// quirk from verifying the 3D render manually) — except where a click
// genuinely has to land on the render itself, which uses canvas-relative
// *fractions*, not raw pixels (see ./helpers.ts), so it survives a
// differently-sized test viewport.

test("starts with a long, real simulated rope already placed", async ({ page }) => {
  await page.goto("/");

  // 24 ch + 1 ss (looping back to stitch 0) + 6 more ch, see
  // lib/starting-rope.ts — a real scheme computed by the same core
  // pipeline as anything else, not a decorative placeholder.
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (31)");
  await expect(page.locator("canvas")).toBeVisible();
  await expect(page.getByTestId("stitch-24")).toContainText("ss -> [0]");
  await expect(page.getByTestId("stat-status")).toHaveText("OK");

  // A real chain already exists, so every kind is available immediately —
  // unlike the true-empty state, which only allows start_ch/mr.
  await expect(page.getByTestId("tool-dc")).toBeEnabled();
  await expect(page.getByTestId("tool-ch")).toBeEnabled();
  await expect(page.getByTestId("tool-mr")).toBeDisabled();
  await expect(page.getByTestId("tool-start_ch")).toBeDisabled();
});

test("Clear returns to a true empty state, with only start_ch and mr available", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (31)");

  await page.getByRole("button", { name: "Clear" }).click();

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (0)");
  await expect(page.getByTestId("tool-start_ch")).toBeEnabled();
  await expect(page.getByTestId("tool-mr")).toBeEnabled();
  for (const kind of ["ch", "ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"]) {
    await expect(page.getByTestId(`tool-${kind}`)).toBeDisabled();
  }
  // The rope's stats must not linger once there's nothing to compute — a
  // real bug caught while building M8's original Clear flow.
  await expect(page.getByTestId("stat-stitches")).toBeHidden();
});

test("remove last undoes the most recent stitch", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (31)");

  await page.getByRole("button", { name: "Remove last" }).click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (30)");
});

test("building a scheme by selecting tools and clicking the render, start to finish", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (0)");

  // start_ch ignores what it hits — any click on the canvas places one
  // (see clickNearOrigin's own comment) — this is what actually exercises
  // the "click the yarn to place a foundation stitch" flow, not just a
  // stand-in for it.
  await page.getByTestId("tool-start_ch").click();
  await clickNearOrigin(page);
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (1)");
  await expect(page.getByTestId("stitch-0")).toContainText("start_ch");

  // mr and start_ch both become unavailable the moment a stitch exists;
  // post stitches stay locked too — a lone start_ch isn't a real chain
  // yet (see lib/tool-placement.ts).
  await expect(page.getByTestId("tool-mr")).toBeDisabled();
  await expect(page.getByTestId("tool-start_ch")).toBeDisabled();
  await expect(page.getByTestId("tool-dc")).toBeDisabled();

  // Placing start_ch deselects itself (it's no longer available), so the
  // next placement needs its own tool selection — ch places via a
  // genuinely empty-space click (Canvas's onPointerMissed), not just a
  // hit on existing geometry.
  await page.getByTestId("tool-ch").click();
  await clickEmptyCorner(page);
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (2)");

  // A target-requiring tool, decrease mode off (the default): a single
  // click on the render places immediately, no confirm click needed.
  // Unlike ch, a miss does nothing (no target-agnostic fallback), so this
  // searches a grid of points rather than betting on one guessed
  // coordinate landing on the actual rendered stitch — see clickUntil's
  // own comment.
  await expect(page.getByTestId("decrease-mode-toggle")).not.toBeChecked();
  await page.getByTestId("tool-dc").click();
  await clickUntil(page, async () => (await page.getByTestId("stitch-count").textContent()) === "Stitches (3)");

  await expect(page.getByTestId("stitch-2")).toContainText("dc ->");
  await expect(page.getByTestId("stat-status")).toBeVisible();
});

test("decrease mode: a target click stays pending until confirmed, instead of placing immediately", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (0)");

  // start_ch, then a real ch (not mr): a lone mr's rendered geometry
  // turned out too small/point-like for clickUntil's sparse grid search
  // to reliably land on (confirmed: consistently missed within its
  // attempt budget), unlike a chain's real line-segment span — start_ch
  // needs a second, real ch before dc unlocks (see lib/tool-placement.ts),
  // so this ends up with two clickable stitches rather than one, but both
  // render the same easy-to-hit chain shape.
  await page.getByTestId("tool-start_ch").click();
  await clickNearOrigin(page);
  await page.getByTestId("tool-ch").click();
  await clickEmptyCorner(page);
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (2)");

  await page.getByTestId("decrease-mode-toggle").check();
  await page.getByTestId("tool-dc").click();
  await expect(page.getByTestId("pending-targets")).toHaveText("No targets selected yet.");

  await clickUntil(page, async () => (await page.getByTestId("pending-targets").textContent())?.includes("click \"dc\" again to place") ?? false);
  // Still just the two foundation stitches — decrease mode holds the
  // click as pending rather than placing immediately.
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (2)");

  await page.getByTestId("tool-dc").click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (3)");
  // Whichever of the two foundation stitches the grid search happened to
  // land on (0 = start_ch, 1 = ch) — the point is that a real target got
  // recorded, not which specific index.
  await expect(page.getByTestId("stitch-2")).toContainText(/dc -> \[[01]\]/);
});
