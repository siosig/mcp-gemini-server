import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "node",
    testTimeout: 60_000, // 60s (accounts for Gemini API latency)
    hookTimeout: 30_000,
    include: ["tests/**/*.test.ts"],
    reporters: ["verbose"],
    pool: "forks",
    poolOptions: {
      forks: {
        singleFork: false,
      },
    },
  },
  esbuild: {
    // esbuild does not recognize ES2025, so fall back to ES2024
    target: "es2024",
  },
  resolve: {
    // ESM extension resolution
    extensions: [".ts", ".js"],
  },
});
