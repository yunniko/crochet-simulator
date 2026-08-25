import { expect, test } from "@playwright/test";

// M5: the editor lets the Owner build a scheme stitch by stitch (not
// just view a hardcoded demo, per M4) and see it computed live through
// the same core -> WASM -> browser pipeline M4 proved end-to-end.
// Asserts via data-testid hooks on the stats/stitch-list — reliable
// DOM-level proxies for "the WASM pipeline actually ran on what the
// editor built" — not on canvas pixels (see HANDOVER.md's note on the
// screenshot-timing quirk from verifying the 3D render manually).

test("default flat-circle preset loads and validates clean", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByTestId("stat-stitches")).toHaveText("7");
  await expect(page.getByTestId("stat-status")).toHaveText("OK");
  await expect(page.locator("canvas")).toBeVisible();
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
  // Asserted explicitly, not just stitch count: an earlier draft of this
  // preset passed a version of this test that only checked stitch count
  // and a target label, while actually rendering "Flagged" in the browser
  // — see HANDOVER.md's M5 entry. This is the regression guard for that.
  await expect(page.getByTestId("stat-status")).toHaveText("OK");
});

test("clearing and adding a stitch by hand updates the scheme", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByText("Add a stitch, or load a preset, to get started.")).toBeVisible();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (0)");

  // Default kind is "dc", no targets selected — a foundation stitch.
  await page.getByRole("button", { name: /Add stitch/ }).click();

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (1)");
  await expect(page.getByTestId("stat-stitches")).toHaveText("1");
  await expect(page.getByTestId("stat-status")).toHaveText("OK");
});

test("remove last undoes the most recent stitch", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (7)");
  await page.getByRole("button", { name: "Remove last" }).click();
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (6)");
});
