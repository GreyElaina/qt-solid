import { spawnSync } from "node:child_process"
import { readFileSync, existsSync } from "node:fs"

import { describe, expect } from "vitest"

import { buildNodeBundle } from "./build-node-bundle.ts"
import { expectCleanExit, nodeBin, projectRoot, stripAnsi, testIfNativeSupported } from "./mocking/native-run"

describe("rolldown app entry", () => {
  testIfNativeSupported("builds export default createApp(...) into self-booting node bundle", async () => {
    const tag = `.tmp-rolldown-app-entry-${process.pid}-${Date.now()}`
    const { bundlePath, cleanup } = await buildNodeBundle({
      bootstrap: true,
      entryExtension: ".tsx",
      entrySource: [
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "export default createApp(() => {",
        "  const mainWindow = createWindow(",
        "    { title: 'rolldown-app', width: 280, height: 160 },",
        "    () => {",
        "      queueMicrotask(() => {",
        "        console.log('APP_MOUNTED')",
        "        process.kill(process.pid, 'SIGTERM')",
        "      })",
        "      return <Text>booted</Text>",
        "    },",
        "  )",
        "",
        "  return mainWindow",
        "})",
      ].join("\n"),
      projectRoot,
      tag,
    })

    try {
      expect(existsSync(`${bundlePath}.map`)).toBe(true)
      expect(readFileSync(bundlePath, "utf8")).toContain("sourceMappingURL=")

      const result = spawnSync(nodeBin, ["--enable-source-maps", "--conditions=browser", bundlePath], {
        cwd: projectRoot,
        encoding: "utf8",
        timeout: 20_000,
      })

      expectCleanExit(result)
      expect(stripAnsi(result.stdout)).toContain("APP_MOUNTED")
      expect(stripAnsi(result.stderr)).not.toContain("qt-solid rolldown bootstrap")
    } finally {
      cleanup()
    }
  })

  testIfNativeSupported("reopens bundled app window on debug activate event", async () => {
    const tag = `.tmp-rolldown-app-activate-${process.pid}-${Date.now()}`
    const { bundlePath, cleanup } = await buildNodeBundle({
      bootstrap: true,
      entryExtension: ".tsx",
      entrySource: [
        "import { __qtSolidDebugEmitAppEvent } from '@qt-solid/core'",
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "let scheduled = false",
        "let reopened = false",
        "",
        "export default createApp(() => {",
        "  const mainWindow = createWindow(",
        "    { title: 'activate-app', width: 300, height: 180 },",
        "    () => <Text>activate</Text>,",
        "  )",
        "",
        "  if (!scheduled) {",
        "    scheduled = true",
        "    setTimeout(() => {",
        "      mainWindow.dispose()",
        "      setTimeout(() => __qtSolidDebugEmitAppEvent('activate'), 20)",
        "    }, 20)",
        "  }",
        "",
        "  return {",
        "    render: () => mainWindow.render(),",
        "    onWindowAllClosed() {},",
        "    onActivate() {",
        "      mainWindow.open()",
        "      if (!reopened) {",
        "        reopened = true",
        "        console.log('APP_REOPENED')",
        "        process.kill(process.pid, 'SIGTERM')",
        "      }",
        "    },",
        "  }",
        "})",
      ].join("\n"),
      projectRoot,
      tag,
    })

    try {
      expect(existsSync(`${bundlePath}.map`)).toBe(true)
      expect(readFileSync(bundlePath, "utf8")).toContain("sourceMappingURL=")

      const result = spawnSync(nodeBin, ["--enable-source-maps", "--conditions=browser", bundlePath], {
        cwd: projectRoot,
        encoding: "utf8",
        timeout: 20_000,
      })

      expectCleanExit(result)
      expect(stripAnsi(result.stdout)).toContain("APP_REOPENED")
    } finally {
      cleanup()
    }
  })

  testIfNativeSupported("resolves devtools worker entry from package root for bundled apps", async () => {
    const tag = `.tmp-rolldown-app-devtools-${process.pid}-${Date.now()}`
    const { bundlePath, cleanup } = await buildNodeBundle({
      bootstrap: true,
      entryExtension: ".tsx",
      entrySource: [
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "let scheduled = false",
        "",
        "export default createApp(() => {",
        "  const mainWindow = createWindow(",
        "    { title: 'devtools-app', width: 280, height: 160 },",
        "    () => <Text>devtools</Text>,",
        "  )",
        "",
        "  if (!scheduled) {",
        "    scheduled = true",
        "    setTimeout(() => {",
        "      console.log('APP_DEVTOOLS_READY')",
        "      process.kill(process.pid, 'SIGTERM')",
        "    }, 150)",
        "  }",
        "",
        "  return mainWindow",
        "})",
      ].join("\n"),
      projectRoot,
      tag,
    })

    try {
      const devtoolsPort = 9500 + Math.floor(Math.random() * 1000)
      const result = spawnSync(nodeBin, ["--enable-source-maps", "--conditions=browser", bundlePath], {
        cwd: projectRoot,
        encoding: "utf8",
        timeout: 20_000,
        env: {
          ...process.env,
          QT_SOLID_DEVTOOLS: "1",
          QT_SOLID_DEVTOOLS_PORT: String(devtoolsPort),
        },
      })

      expectCleanExit(result)
      const stdout = stripAnsi(result.stdout)
      expect(stdout).toContain("APP_DEVTOOLS_READY")
      expect(stdout).toContain(`[qt-solid devtools] http://127.0.0.1:${devtoolsPort}/json/list`)
      expect(stripAnsi(result.stderr)).not.toContain("Cannot find module")
      expect(stripAnsi(result.stderr)).not.toContain("cdp-worker.mjs")
    } finally {
      cleanup()
    }
  })
})
