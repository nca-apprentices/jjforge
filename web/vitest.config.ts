import { defineConfig } from "vitest/config";

// One run covers the tests of every package in the workspace.
export default defineConfig({
  test: {
    include: ["{apps,features,shared}/*/src/**/*.test.{ts,tsx}"],
  },
});
