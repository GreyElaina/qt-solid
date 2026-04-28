import { defineConfig } from "vitest/config"
import { fileURLToPath } from "node:url"

import qtSolidVitePlugin from "@qt-solid/solid/vite"

const fakeExampleWidgetsNativePath = fileURLToPath(
  new URL("./test/mocking/fake-example-widgets-native.ts", import.meta.url),
)

export default defineConfig({
  plugins: [qtSolidVitePlugin],
  resolve: {
    conditions: ["browser"],
    alias: [
      {
        find: /^solid-js$/,
        replacement: "solid-js/dist/solid.js",
      },
      {
        find: /^solid-js\/store$/,
        replacement: "solid-js/store/dist/store.js",
      },
      {
        find: /^@qt-solid\/example-widgets\/native$/,
        replacement: fakeExampleWidgetsNativePath,
      },
    ],
  },
  test: {
    environment: "node",
    fileParallelism: false,
    include: ["test/**/*.test.ts", "test/**/*.test.tsx"],
  },
})
