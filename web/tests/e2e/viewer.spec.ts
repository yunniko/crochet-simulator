import { expect, test } from "@playwright/test";

// M4: proves the whole pipeline end-to-end in a real browser — WASM
// computation (core/ -> wasm/), the JS bridge (lib/wasm), and rendering
// (components/YarnViewer) — not just that each piece works in isolation.
// Asserts on the stats text (a DOM-level, reliably-testable proxy for
// "the WASM module actually ran and produced the expected result"),
// not on canvas pixels: verifying the *3D render itself* looks right
// needs actual human/visual review — see HANDOVER.md's note on the
// screenshot-timing quirk hit while doing that manually for this
// milestone. A canvas element's mere presence is checked below as a
// basic sanity signal that the viewer mounted at all.

test("flat circle demo loads and validates clean", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Stitches:")).toBeVisible();
  await expect(page.getByText("7", { exact: true })).toBeVisible();
  await expect(page.getByText("OK", { exact: true })).toBeVisible();
  await expect(page.locator("canvas")).toBeVisible();
});

test("switching to the overloaded demo shows it flagged", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Overloaded ring (flagged)" }).click();

  await expect(page.getByText("16", { exact: true })).toBeVisible();
  await expect(page.getByText(/Flagged \(\d+ intersections?\)/)).toBeVisible();
});

test("switching back to the flat circle demo returns to a clean state", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Overloaded ring (flagged)" }).click();
  await expect(page.getByText(/Flagged \(\d+ intersections?\)/)).toBeVisible();

  await page.getByRole("button", { name: "Flat circle (valid)" }).click();
  await expect(page.getByText("OK", { exact: true })).toBeVisible();
});
