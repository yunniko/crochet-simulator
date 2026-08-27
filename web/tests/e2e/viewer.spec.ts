import { expect, test } from "@playwright/test";

import { clickEmptyCorner, clickNearOrigin, clickUntil } from "./helpers";

// M8: the editor is a direct-manipulation, click-on-the-render tool
// palette, not a form — the app starts empty (a plain starting yarn stub,
// no scheme yet) and every stitch is placed by selecting a tool and
// clicking the 3D view. Asserts via data-testid hooks on the tool
// palette / pending-target hint / stats — not on canvas pixels (see
// HANDOVER.md's note on the screenshot-timing quirk from verifying the
// 3D render manually) — except where a click genuinely has to land on
// the render itself, which uses canvas-relative *fractions*, not raw
// pixels (see ./helpers.ts), so it survives a differently-sized test
// viewport.

test("starts empty, with only ch and mr available", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (0)");
  await expect(page.locator("canvas")).toBeVisible();
  await expect(page.getByTestId("tool-ch")).toBeEnabled();
  await expect(page.getByTestId("tool-mr")).toBeEnabled();
  for (const kind of ["ss", "dc", "htr", "tr", "dtr", "trtr", "quad_tr"]) {
    await expect(page.getByTestId(`tool-${kind}`)).toBeDisabled();
  }
});

test("loading the flat-circle preset validates clean", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Flat circle (round 1)" }).click();

  await expect(page.getByTestId("stat-stitches")).toHaveText("7");
  await expect(page.getByTestId("stat-status")).toHaveText("OK");
});

test("switching to the overloaded ring preset shows it flagged", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Overloaded ring (flagged)" }).click();

  await expect(page.getByTestId("stat-stitches")).toHaveText("16");
  await expect(page.getByTestId("stat-status")).toContainText("Flagged");
});

test("the freeform spike preset (non-row targeting) validates clean", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Freeform spike (non-row)" }).click();

  await expect(page.getByTestId("stat-stitches")).toHaveText("5");
  // Stitch 4 targets stitch 0 (not its immediate predecessor, stitch 3) —
  // the concrete evidence this scheme really isn't row-based.
  await expect(page.getByTestId("stitch-4")).toContainText("dc -> [0]");
  await expect(page.getByTestId("stat-status")).toHaveText("OK");
});

test("clearing a loaded preset returns to the empty starting state", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Flat circle (round 1)" }).click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (7)");

  await page.getByRole("button", { name: "Clear" }).click();

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (0)");
  await expect(page.getByTestId("tool-mr")).toBeEnabled();
  // The last scheme's stats must not linger once there's nothing to
  // compute — a real bug caught while building this milestone (a Clear
  // left a stale "Flagged" reading on screen).
  await expect(page.getByTestId("stat-stitches")).toBeHidden();
});

test("remove last undoes the most recent stitch", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Flat circle (round 1)" }).click();

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (7)");
  await page.getByRole("button", { name: "Remove last" }).click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (6)");
});

test("building a scheme by selecting tools and clicking the render, start to finish", async ({ page }) => {
  await page.goto("/");

  // ch ignores what it hits — any click on the canvas places one (see
  // clickNearOrigin's own comment) — this is what actually exercises the
  // "click the yarn to place a foundation stitch" flow, not just a
  // stand-in for it.
  await page.getByTestId("tool-ch").click();
  await clickNearOrigin(page);
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (1)");
  await expect(page.getByTestId("stitch-0")).toContainText("ch");

  // mr becomes unavailable the moment a stitch exists.
  await expect(page.getByTestId("tool-mr")).toBeDisabled();

  // ch also places via a genuinely empty-space click (Canvas's
  // onPointerMissed), not just a hit on existing geometry.
  await clickEmptyCorner(page);
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (2)");

  // A target-requiring tool: click the render to select a target, then
  // the tool button again to confirm and place. Unlike ch, a miss here
  // does nothing (no target-agnostic fallback), so this searches a grid
  // of points rather than betting on one guessed coordinate landing on
  // the actual rendered stitch — see clickUntil's own comment.
  await page.getByTestId("tool-dc").click();
  await expect(page.getByTestId("pending-targets")).toHaveText("No targets selected yet.");
  await clickUntil(page, async () => (await page.getByTestId("pending-targets").textContent())?.includes("click \"dc\" again to place") ?? false);
  await page.getByTestId("tool-dc").click();

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (3)");
  await expect(page.getByTestId("stitch-2")).toContainText("dc ->");
  await expect(page.getByTestId("stat-status")).toBeVisible();
});
