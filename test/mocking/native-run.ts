import { spawnSync, type SpawnSyncReturns } from "node:child_process"
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { fileURLToPath } from "node:url"

import { expect, test, type TestFunction } from "vitest"

import { buildNodeBundle } from "../build-node-bundle.ts"

export const nodeBin = "/opt/homebrew/opt/node/bin/node"
export const projectRoot = fileURLToPath(new URL("../..", import.meta.url))
export const nativeModuleSpecifier = "@qt-solid/core"
export const coreWidgetsNativeModuleSpecifier = "@qt-solid/core-widgets/native"
const nativeTestTimeoutMs = 60_000

const nativeSupported = process.platform === "darwin" && existsSync(nodeBin)

type NativeSupportedTest = (
  name: string | Function,
  fn?: TestFunction,
  timeout?: number,
) => void

export const testIfNativeSupported: NativeSupportedTest = nativeSupported
  ? (name, fn, timeout) => test(name, fn, timeout ?? nativeTestTimeoutMs)
  : (name, fn, timeout) => test.skip(name, fn, timeout)

export interface NativeBundleRunOptions {
  entryExtension: ".ts" | ".tsx"
  entrySource: string
  tagPrefix: string
  widgetLibraries?: readonly string[]
}

export function stripAnsi(value: string) {
  return value.replace(/\u001b\[[0-9;]*m/g, "")
}

export function expectCleanExit(result: SpawnSyncReturns<string>) {
  expect(result.error).toBeUndefined()
  expect(result.signal).toBeNull()
  expect(result.status).toBe(0)
  expect(result.stderr).not.toContain("Segmentation fault")
  expect(result.stderr).not.toContain("EXC_BAD_ACCESS")
  expect(result.stderr).not.toContain("QThreadStorage:")
}

export function runNodeScript(source: string) {
  const tempDir = mkdtempSync(join(projectRoot, ".tmp-qt-solid-native-"))
  const scriptPath = join(tempDir, "script.mjs")
  writeFileSync(scriptPath, source)

  try {
    return spawnSync(nodeBin, ["--conditions=browser", scriptPath], {
      cwd: projectRoot,
      encoding: "utf8",
      timeout: 20_000,
    })
  } finally {
    rmSync(tempDir, { force: true, recursive: true })
  }
}

export async function runBundledNodeScript(options: NativeBundleRunOptions) {
  const tag = `${options.tagPrefix}-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}`
  const tempDir = mkdtempSync(join(projectRoot, ".tmp-qt-solid-native-bundle-"))
  const scriptPath = join(tempDir, "script.mjs")

  const { bundlePath, cleanup } = await buildNodeBundle({
    projectRoot,
    tag,
    entryExtension: options.entryExtension,
    entrySource: options.entrySource,
    widgetLibraries: options.widgetLibraries,
  })

  try {
    writeFileSync(
      scriptPath,
      [
        `import { QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { run } from ${JSON.stringify(new URL(`file://${bundlePath}`).href)}`,
        "",
        "const app = QtApp.start(() => {})",
        "await run(app)",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"),
    )

    return spawnSync(nodeBin, ["--conditions=browser", scriptPath], {
      cwd: projectRoot,
      encoding: "utf8",
      timeout: 20_000,
    })
  } finally {
    cleanup()
    rmSync(tempDir, { force: true, recursive: true })
  }
}

export function parseSnapshot<T>(stdout: string): T {
  const match = stripAnsi(stdout).match(/SNAPSHOT (\{.*\})/)
  expect(match).not.toBeNull()
  if (!match?.[1]) {
    throw new Error("missing snapshot payload")
  }
  return JSON.parse(match[1]) as T
}
