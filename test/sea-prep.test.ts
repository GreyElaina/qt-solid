import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"

import { describe, expect } from "vitest"

import {
  buildSolidNodeSeaExecutable,
  buildSolidNodeSeaPrep,
} from "../packages/solid/src/build/build-solid-node-sea.js"
import { nodeBin, projectRoot, testIfNativeSupported } from "./mocking/native-run"

describe("node SEA prep", () => {
  testIfNativeSupported("builds qt-solid counter app into SEA prep blob with runtime assets", async () => {
    const tempDir = mkdtempSync(join(projectRoot, ".tmp-qt-solid-sea-"))
    const entryPath = join(tempDir, "app.tsx")
    const outDir = join(tempDir, "dist-sea")

    writeFileSync(
      entryPath,
      [
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "export default createApp(() => {",
        "  return createWindow({ title: 'sea-test', width: 240, height: 120 }, () => <Text>sea</Text>)",
        "})",
      ].join("\n"),
    )

    try {
      const result = await buildSolidNodeSeaPrep({
        entryPath,
        outDir,
        nodeBinary: nodeBin,
        widgetLibraries: ["@qt-solid/core-widgets/widget-library"],
      })

      expect(existsSync(result.bundlePath)).toBe(true)
      expect(existsSync(result.prepBlobPath)).toBe(true)
      expect(existsSync(result.seaConfigPath)).toBe(true)
      expect(existsSync(join(outDir, "sea-main.cjs"))).toBe(true)

      const config = JSON.parse(readFileSync(result.seaConfigPath, "utf8"))
      expect(config.useSnapshot).toBe(false)
      expect(config.useCodeCache).toBe(false)
      expect(config.execArgv).toEqual(["--enable-source-maps", "--conditions=browser"])
      expect(config.assets["app/app.mjs"]).toBe(result.bundlePath)
      expect(config.assets["node_modules/@qt-solid/core/package.json"]).toBe(
        join(outDir, "node_modules/@qt-solid/core/package.json"),
      )
      expect(config.assets["node_modules/@qt-solid/core/native/index.js"]).toBe(
        join(outDir, "node_modules/@qt-solid/core/native/index.js"),
      )
      expect(config.assets["node_modules/@qt-solid/core-widgets/package.json"]).toBe(
        join(outDir, "node_modules/@qt-solid/core-widgets/package.json"),
      )
      expect(config.assets["app/cdp-worker.mjs"]).toBe(join(outDir, "app/cdp-worker.mjs"))
    } finally {
      rmSync(tempDir, { force: true, recursive: true })
    }
  })

  testIfNativeSupported("reports executable capability when direct SEA binary build is unavailable", async () => {
    const tempDir = mkdtempSync(join(projectRoot, ".tmp-qt-solid-sea-exe-"))
    const entryPath = join(tempDir, "app.tsx")
    const outDir = join(tempDir, "dist-sea")

    writeFileSync(
      entryPath,
      [
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "export default createApp(() => {",
        "  return createWindow({ title: 'sea-exe-test', width: 200, height: 100 }, () => <Text>sea</Text>)",
        "})",
      ].join("\n"),
    )

    try {
      const result = await buildSolidNodeSeaExecutable({
        entryPath,
        outDir,
        nodeBinary: nodeBin,
        widgetLibraries: ["@qt-solid/core-widgets/widget-library"],
      })

      expect(existsSync(result.prepBlobPath)).toBe(true)
      if (result.executablePath) {
        expect(existsSync(result.executablePath)).toBe(true)
        expect(result.capabilityError).toBeNull()
      } else {
        expect(result.capabilityError).toContain("NODE_SEA_FUSE")
      }
    } finally {
      rmSync(tempDir, { force: true, recursive: true })
    }
  })
})
