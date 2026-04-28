import { fileURLToPath } from "node:url"

import { defineConfig } from "rolldown"
import qtSolidRolldownPlugin from "@qt-solid/solid/rolldown"

export default defineConfig({
  input: fileURLToPath(new URL("./app.tsx", import.meta.url)),
  output: {
    file: fileURLToPath(new URL("./dist/app.js", import.meta.url)),
    format: "esm",
    sourcemap: true,
  },
  platform: "node",
  plugins: [qtSolidRolldownPlugin],
  resolve: {
    conditionNames: ["browser"],
  },
})
