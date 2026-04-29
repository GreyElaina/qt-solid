import { defineConfig } from "@qt-solid/solid/preview/config"

export default defineConfig({
  theme: "./packages/fluent/src/theme.ts",
  components: [
    "./packages/fluent/src/index.ts",
  ],
  preview: {
    width: 400,
    height: 300,
    wsPort: 9230,
  },
})
