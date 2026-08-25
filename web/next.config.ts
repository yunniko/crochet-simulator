import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Playwright (playwright.config.ts) drives the dev server via
  // 127.0.0.1, which Next's dev-origin check otherwise treats as a
  // different, untrusted origin from `localhost` and blocks (silently
  // 403s every JS chunk, which just hangs the app instead of erroring
  // visibly — found by capturing console/network activity during a
  // failing e2e run, not obvious from the app's own behaviour alone).
  allowedDevOrigins: ["127.0.0.1"],
};

export default nextConfig;
