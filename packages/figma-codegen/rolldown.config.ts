import { defineConfig } from "rolldown"

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
})
