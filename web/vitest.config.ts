import path from "node:path";
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // e2e specs under tests/e2e belong to Playwright, not Vitest
    include: ["tests/unit/**/*.spec.ts"],
  },
  resolve: {
    alias: { "@": path.resolve(__dirname, ".") },
  },
});
