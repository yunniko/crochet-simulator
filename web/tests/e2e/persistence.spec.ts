import { expect, test } from "@playwright/test";

// M6: save/load. Hits the real dev Postgres (docker compose's `db`
// service, see ../../README.md) through the actual server actions — not
// mocked — since the whole point is proving the browser -> server action
// -> Prisma -> Postgres round trip works, matching the Company standard
// of verifying by running, not just asserting in isolation.

test("saving a scheme updates the URL and share link, and reloading it round-trips the scheme", async ({
  page,
}) => {
  await page.goto("/");

  // Start from a small, deterministic scheme rather than a preset, so the
  // round-trip assertion below isn't just re-checking a preset's own data.
  await page.getByRole("button", { name: "Clear" }).click();
  await page.getByRole("button", { name: /Add stitch/ }).click(); // [0] dc, no targets

  await page.getByTestId("scheme-name-input").fill("e2e persistence test");
  await page.getByTestId("save-button").click();

  // URL updates in place (history.replaceState, no reload) to /s/<slug>.
  await expect(page).toHaveURL(/\/s\/[a-z0-9]{12}$/);
  await expect(page.getByTestId("save-button")).toHaveText("Save changes");
  const shareLink = page.getByTestId("share-link");
  await expect(shareLink).toBeVisible();
  const shareUrl = await shareLink.getAttribute("title");
  expect(shareUrl).toMatch(/\/s\/[a-z0-9]{12}$/);

  // A genuinely fresh load (new navigation, not client-side state) of that
  // same URL must restore the same scheme — the actual point of M6.
  const slugUrl = page.url();
  await page.goto("about:blank");
  await page.goto(slugUrl);

  await expect(page.getByTestId("scheme-name-input")).toHaveValue("e2e persistence test");
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (1)");
  await expect(page.getByTestId("stat-status")).toHaveText("OK");
  await expect(page.getByTestId("save-button")).toHaveText("Save changes");
});

test("saving again on an already-saved scheme overwrites it in place, not a new link", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Clear" }).click();
  await page.getByRole("button", { name: /Add stitch/ }).click();
  await page.getByTestId("save-button").click();
  await expect(page).toHaveURL(/\/s\/[a-z0-9]{12}$/);
  const firstUrl = page.url();

  await page.getByRole("button", { name: /Add stitch/ }).click(); // now 2 stitches
  // Unlike the first save, neither the URL nor the client-side stitch
  // count changes as a *result* of this second save completing (the URL
  // was already firstUrl, and stat-stitches already reflects the local
  // 2-stitch state before the server round trip finishes) — so there's
  // nothing to auto-wait on. Wait for the actual save request/response
  // instead, or the next assertion can race ahead of the write actually
  // committing (caught concretely: the DB row was correct, but a
  // `page.goto` fired before the save's await had resolved still read the
  // pre-write row).
  await Promise.all([page.waitForResponse((r) => r.request().method() === "POST"), page.getByTestId("save-button").click()]);

  await expect(page).toHaveURL(firstUrl);
  await expect(page.getByTestId("stat-stitches")).toHaveText("2");

  await page.goto(firstUrl);
  await expect(page.getByTestId("stitch-count")).toHaveText("Stitches (2)");
});

test("visiting an unknown scheme link shows a not-found page", async ({ page }) => {
  const response = await page.goto("/s/000000000000");
  expect(response?.status()).toBe(404);
});
