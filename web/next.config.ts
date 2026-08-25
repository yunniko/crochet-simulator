import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Playwright (playwright.config.ts) drives the dev server via
  // 127.0.0.1, which Next's dev-origin check otherwise treats as a
  // different, untrusted origin from `localhost` and blocks (silently
  // 403s every JS chunk, which just hangs the app instead of erroring
  // visibly — found by capturing console/network activity during a
  // failing e2e run, not obvious from the app's own behaviour alone).
  allowedDevOrigins: ["127.0.0.1"],
  // M6: needed for the Docker production image (web/Dockerfile copies
  // .next/standalone) — matches the portfolio's other Next.js deploys.
  output: "standalone",
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          // Nothing about this app benefits from being iframed, and its
          // one-click action (overwriting a saved scheme) is exactly what
          // clickjacking targets — block framing outright, matching the
          // portfolio's other deployed apps.
          { key: "X-Frame-Options", value: "DENY" },
          { key: "Content-Security-Policy", value: "frame-ancestors 'none'" },
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
        ],
      },
    ];
  },
};

export default nextConfig;
