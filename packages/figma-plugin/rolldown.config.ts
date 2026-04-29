import { defineConfig } from "rolldown"
import { copyFileSync, mkdirSync } from "node:fs"

export default defineConfig({
  input: "src/code.ts",
  output: {
    file: "dist/code.js",
    format: "iife",
  },
  platform: "browser",
  transform: {
    target: "es2017",
  },
  plugins: [
    {
      name: "copy-ui",
      buildEnd() {
        mkdirSync("dist", { recursive: true })
        copyFileSync("src/ui.html", "dist/ui.html")
      },
    },
  ],
})
