import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { join, relative } from "node:path"
import { createInterface } from "node:readline"

import { afterEach, describe, expect } from "vitest"

import { nodeBin, projectRoot, stripAnsi, testIfNativeSupported } from "./mocking/native-run"

interface ProbeMemoryUsage {
  arrayBuffers: number
  external: number
  heapTotal: number
  heapUsed: number
  rss: number
}

interface ProbeMessage {
  type: "probe"
  fixtureState: {
    disposals: string[]
    loads: string[]
  }
  label: string
  pid: number
  heapSpaces: Record<string, number>
  memoryAfterGc: ProbeMemoryUsage
  memoryBeforeGc: ProbeMemoryUsage
  nodeCount: number
  nodeKinds: Record<string, number>
  texts: string[]
  viteState: {
    environmentFiles: number
    environmentIds: number
    environmentUrls: number
    runnerFiles: number
    runnerIds: number
    runnerUrls: number
  }
}

interface HeapSnapshotMessage {
  type: "heap-snapshot"
  label: string
  path: string
}

type ChildMessage = ProbeMessage | HeapSnapshotMessage

function sleep(delayMs: number) {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, delayMs)
  })
}

function createViewSource(version: string): string {
  return [
    "import type { Component } from 'solid-js'",
    "import { Button, Column, Text } from '@qt-solid/solid'",
    "",
    `const version = ${JSON.stringify(version)}`,
    "const markerPrefix = ['qt', 'solid', 'hmr', 'payload', version].join('-')",
    "const runtimePayload = Array.from({ length: 256 }, (_, index) => `${markerPrefix}-${index}`).join('|')",
    "const fixtureState = ((globalThis as typeof globalThis & {",
    "  __qtSolidHmrFixture__?: {",
    "    disposals: string[]",
    "    loads: string[]",
    "  }",
    "}).__qtSolidHmrFixture__ ??= {",
    "  disposals: [],",
    "  loads: [],",
    "})",
    "fixtureState.loads.push(version)",
    "",
    "if (import.meta.hot) {",
    "  import.meta.hot.dispose(() => {",
    "    fixtureState.disposals.push(version)",
    "  })",
    "}",
    "",
    "export const CounterView: Component = () => {",
    "  return (",
    "    <Column gap={8} padding={8}>",
    `      <Text>hmr version ${version}</Text>`,
    "      <Text>{runtimePayload.slice(0, 16)}</Text>",
    "      <Button>noop</Button>",
    "    </Column>",
    "  )",
    "}",
  ].join("\n")
}

function createAppSource(version: string): string {
  return [
    "/// <reference types='vite/client' />",
    "",
    "import { acceptQtSolidDevAppHmr } from '@qt-solid/solid/hmr'",
    "import { Column, createApp, createWindow, Text } from '@qt-solid/solid'",
    "",
    "import { CounterView } from './view.tsx'",
    "",
    `const appVersion = ${JSON.stringify(version)}`,
    "",
    "const app = createApp(() => {",
    "  const mainWindow = createWindow(",
    "    {",
    "      title: 'vite-hmr-memory',",
    "      width: 320,",
    "      height: 180,",
    "    },",
    "    () => (",
    "      <Column gap={12} padding={12}>",
    "        <Text>{`app version ${appVersion}`}</Text>",
    "        <CounterView />",
    "      </Column>",
    "    ),",
    "  )",
    "",
    "  return mainWindow",
    "})",
    "",
    "acceptQtSolidDevAppHmr(import.meta, app)",
    "",
    "export default app",
  ].join("\n")
}

function createProbeSource(): string {
  return [
    "import { createInterface } from 'node:readline'",
    "import { join } from 'node:path'",
    "import { tmpdir } from 'node:os'",
    "import * as v8 from 'node:v8'",
    "",
    "interface QtDebugNodeSnapshot {",
    "  kind: string",
    "  text?: string | null",
    "}",
    "",
    "interface QtDebugSnapshot {",
    "  nodes: QtDebugNodeSnapshot[]",
    "}",
    "",
    "interface DevSessionState {",
    "  app?: {",
    "    debugSnapshot(): QtDebugSnapshot",
    "  }",
    "}",
    "",
    "interface FixtureState {",
    "  disposals: string[]",
    "  loads: string[]",
    "}",
    "",
    "let started = false",
    "",
    "function emit(message: unknown): void {",
    "  process.stdout.write(`${JSON.stringify(message)}\\n`)",
    "}",
    "",
    "function currentSnapshot(): QtDebugSnapshot | null {",
    "  const state = (globalThis as typeof globalThis & {",
    "    __qtSolidDevSession__?: DevSessionState",
    "  }).__qtSolidDevSession__",
    "",
    "  try {",
    "    return state?.app?.debugSnapshot() ?? null",
    "  } catch {",
    "    return null",
    "  }",
    "}",
    "",
    "function currentFixtureState(): FixtureState {",
    "  return ((globalThis as typeof globalThis & {",
    "    __qtSolidHmrFixture__?: FixtureState",
    "  }).__qtSolidHmrFixture__ ??= {",
    "    disposals: [],",
    "    loads: [],",
    "  })",
    "}",
    "",
    "function collectHeapSpaces(): Record<string, number> {",
    "  return Object.fromEntries(",
    "    v8.getHeapSpaceStatistics().map((space) => [space.space_name, space.space_used_size]),",
    "  )",
    "}",
    "",
    "function currentViteState(): {",
    "  environmentFiles: number",
    "  environmentIds: number",
    "  environmentUrls: number",
    "  runnerFiles: number",
    "  runnerIds: number",
    "  runnerUrls: number",
    "} {",
    "  const environment = (globalThis as typeof globalThis & {",
    "    __qtSolidViteDevEnvironment__?: {",
    "      moduleGraph?: {",
    "        fileToModulesMap?: Map<string, unknown>",
    "        idToModuleMap?: Map<string, unknown>",
    "        urlToModuleMap?: Map<string, unknown>",
    "      }",
    "      runner?: {",
    "        evaluatedModules?: {",
    "          fileToModulesMap?: Map<string, unknown>",
    "          idToModuleMap?: Map<string, unknown>",
    "          urlToIdModuleMap?: Map<string, unknown>",
    "        }",
    "      }",
    "    }",
    "  }).__qtSolidViteDevEnvironment__",
    "",
    "  return {",
    "    environmentFiles: environment?.moduleGraph?.fileToModulesMap?.size ?? 0,",
    "    environmentIds: environment?.moduleGraph?.idToModuleMap?.size ?? 0,",
    "    environmentUrls: environment?.moduleGraph?.urlToModuleMap?.size ?? 0,",
    "    runnerFiles: environment?.runner?.evaluatedModules?.fileToModulesMap?.size ?? 0,",
    "    runnerIds: environment?.runner?.evaluatedModules?.idToModuleMap?.size ?? 0,",
    "    runnerUrls: environment?.runner?.evaluatedModules?.urlToIdModuleMap?.size ?? 0,",
    "  }",
    "}",
    "",
    "function collectProbe(label: string): void {",
    "  const snapshot = currentSnapshot()",
    "  const memoryBeforeGc = process.memoryUsage()",
    "  if (typeof global.gc === 'function') {",
    "    global.gc()",
    "  }",
    "  const memoryAfterGc = process.memoryUsage()",
    "",
    "  const texts = snapshot?.nodes",
    "    .filter((node): node is QtDebugNodeSnapshot & { text: string } => node.kind === 'text' && typeof node.text === 'string')",
    "    .map((node) => node.text) ?? []",
    "",
    "  const nodeKinds = snapshot?.nodes.reduce<Record<string, number>>((counts, node) => {",
    "    counts[node.kind] = (counts[node.kind] ?? 0) + 1",
    "    return counts",
    "  }, {}) ?? {}",
    "",
    "  emit({",
    "    type: 'probe',",
    "    fixtureState: currentFixtureState(),",
    "    label,",
    "    pid: process.pid,",
    "    heapSpaces: collectHeapSpaces(),",
    "    memoryAfterGc,",
    "    memoryBeforeGc,",
    "    nodeCount: snapshot?.nodes.length ?? 0,",
    "    nodeKinds,",
    "    texts,",
    "    viteState: currentViteState(),",
    "  })",
    "}",
    "",
    "async function emitReadyProbe(): Promise<void> {",
    "  for (let attempt = 0; attempt < 80; attempt += 1) {",
    "    const snapshot = currentSnapshot()",
    "    if (snapshot && snapshot.nodes.length > 1) {",
    "      collectProbe('ready')",
    "      return",
    "    }",
    "    await new Promise<void>((resolve) => setTimeout(resolve, 25))",
    "  }",
    "",
    "  collectProbe('ready')",
    "}",
    "",
    "export function startProbeServer(): void {",
    "  if (started) {",
    "    return",
    "  }",
    "",
    "  started = true",
    "  void emitReadyProbe()",
    "",
    "  const lines = createInterface({",
    "    input: process.stdin,",
    "    crlfDelay: Infinity,",
    "  })",
    "",
    "  lines.on('line', (line) => {",
    "    let message: { type?: string; label?: string }",
    "    try {",
    "      message = JSON.parse(line) as { type?: string; label?: string }",
    "    } catch {",
    "      return",
    "    }",
    "",
    "    if (message.type === 'probe') {",
    "      queueMicrotask(() => collectProbe(String(message.label ?? 'probe')))",
    "      return",
    "    }",
    "",
    "    if (message.type === 'heap-snapshot') {",
    "      queueMicrotask(() => {",
    "        const label = String(message.label ?? 'snapshot')",
    "        const path = v8.writeHeapSnapshot(",
    "          join(tmpdir(), `qt-solid-hmr-${process.pid}-${label}-${Date.now()}.heapsnapshot`),",
    "        )",
    "        emit({",
    "          type: 'heap-snapshot',",
    "          label,",
    "          path,",
    "        })",
    "      })",
    "      return",
    "    }",
    "",
    "    if (message.type === 'shutdown') {",
    "      process.kill(process.pid, 'SIGTERM')",
    "    }",
    "  })",
    "}",
  ].join("\n")
}

function createDevEntrySource(): string {
  return [
    "import { mountOrReplaceQtSolidDevApp } from '@qt-solid/solid/hmr'",
    "",
    "import app from './app.tsx'",
    "import { startProbeServer } from './probe.ts'",
    "",
    "mountOrReplaceQtSolidDevApp(app)",
    "",
    "startProbeServer()",
  ].join("\n")
}

function createFixture(): { appPath: string; dir: string; entryPath: string; viewPath: string } {
  const dir = mkdtempSync(join(projectRoot, ".tmp-vite-hmr-memory-"))
  const appPath = join(dir, "app.tsx")
  const viewPath = join(dir, "view.tsx")
  const entryPath = join(dir, "dev-entry.tsx")

  writeFileSync(viewPath, createViewSource("0"))
  writeFileSync(appPath, createAppSource("0"))
  writeFileSync(join(dir, "probe.ts"), createProbeSource())
  writeFileSync(entryPath, createDevEntrySource())

  return { appPath, dir, entryPath, viewPath }
}

function formatOutput(stdoutLines: string[], stderrLines: string[]): string {
  return `stdout:\n${stripAnsi(stdoutLines.join("\n"))}\n\nstderr:\n${stripAnsi(stderrLines.join("\n"))}`
}

async function terminateChild(child: ChildProcessWithoutNullStreams | null): Promise<void> {
  if (!child || child.killed) {
    return
  }

  if (child.exitCode != null || child.signalCode != null) {
    return
  }

  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL")
    }, 2_000)

    child.once("exit", () => {
      clearTimeout(timer)
      resolve()
    })

    child.kill("SIGTERM")
  })
}

class HmrChildController {
  private readonly stdoutLines: string[] = []
  private readonly stderrLines: string[] = []
  private readonly messages: ChildMessage[] = []
  private readonly waiters = new Set<{
    predicate: (message: ChildMessage) => boolean
    reject: (error: Error) => void
    resolve: (message: ChildMessage) => void
    timer: ReturnType<typeof setTimeout>
  }>()

  constructor(private readonly child: ChildProcessWithoutNullStreams) {
    const stdout = createInterface({ input: child.stdout })
    const stderr = createInterface({ input: child.stderr })

    stdout.on("line", (line) => {
      this.stdoutLines.push(line)

      let message: ChildMessage
      try {
        message = JSON.parse(line) as ChildMessage
      } catch {
        return
      }

      for (const waiter of this.waiters) {
        if (!waiter.predicate(message)) {
          continue
        }

        this.waiters.delete(waiter)
        clearTimeout(waiter.timer)
        waiter.resolve(message)
        return
      }

      this.messages.push(message)
    })

    stderr.on("line", (line) => {
      this.stderrLines.push(line)
    })

    child.once("exit", (code, signal) => {
      const error = new Error(
        `HMR child exited early (code=${String(code)} signal=${String(signal)})\n${formatOutput(this.stdoutLines, this.stderrLines)}`,
      )

      for (const waiter of this.waiters) {
        clearTimeout(waiter.timer)
        waiter.reject(error)
      }
      this.waiters.clear()
    })
  }

  async waitForProbe(label: string, timeoutMs = 10_000): Promise<ProbeMessage> {
    const existing = this.messages.find(
      (message): message is ProbeMessage => message.type === "probe" && message.label === label,
    )
    if (existing) {
      return existing
    }

    return await new Promise<ProbeMessage>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiters.delete(waiter)
        reject(new Error(`Timed out waiting for probe ${label}\n${formatOutput(this.stdoutLines, this.stderrLines)}`))
      }, timeoutMs)

      const waiter = {
        predicate: (message: ChildMessage) => message.type === "probe" && message.label === label,
        reject,
        resolve: (message: ChildMessage) => resolve(message as ProbeMessage),
        timer,
      }

      this.waiters.add(waiter)
    })
  }

  async requestProbe(label: string, timeoutMs = 10_000): Promise<ProbeMessage> {
    this.child.stdin.write(`${JSON.stringify({ type: "probe", label })}\n`)
    return await this.waitForProbe(label, timeoutMs)
  }

  async waitForHeapSnapshot(label: string, timeoutMs = 60_000): Promise<HeapSnapshotMessage> {
    const existing = this.messages.find(
      (message): message is HeapSnapshotMessage => message.type === "heap-snapshot" && message.label === label,
    )
    if (existing) {
      return existing
    }

    return await new Promise<HeapSnapshotMessage>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiters.delete(waiter)
        reject(
          new Error(`Timed out waiting for heap snapshot ${label}\n${formatOutput(this.stdoutLines, this.stderrLines)}`),
        )
      }, timeoutMs)

      const waiter = {
        predicate: (message: ChildMessage) => message.type === "heap-snapshot" && message.label === label,
        reject,
        resolve: (message: ChildMessage) => resolve(message as HeapSnapshotMessage),
        timer,
      }

      this.waiters.add(waiter)
    })
  }

  async requestHeapSnapshot(label: string, timeoutMs = 60_000): Promise<HeapSnapshotMessage> {
    this.child.stdin.write(`${JSON.stringify({ type: "heap-snapshot", label })}\n`)
    return await this.waitForHeapSnapshot(label, timeoutMs)
  }

  async waitForRenderedText(text: string, labelPrefix: string, attempts = 40): Promise<ProbeMessage> {
    for (let attempt = 0; attempt < attempts; attempt += 1) {
      const probe = await this.requestProbe(`${labelPrefix}-${attempt}`)
      if (probe.texts.includes(text)) {
        return probe
      }

      await sleep(100)
    }

    throw new Error(`Timed out waiting for rendered text ${text}\n${formatOutput(this.stdoutLines, this.stderrLines)}`)
  }

  async close(): Promise<void> {
    if (!this.child.killed) {
      this.child.stdin.write(`${JSON.stringify({ type: "shutdown" })}\n`)
    }
    await terminateChild(this.child)
  }
}

function readMarkerPresence(snapshotPath: string, markers: string[]): Record<string, boolean> {
  try {
    const snapshotText = readFileSync(snapshotPath, "utf8")
    return Object.fromEntries(markers.map((marker) => [marker, snapshotText.includes(marker)]))
  } finally {
    rmSync(snapshotPath, { force: true })
  }
}

async function spawnHmrChild(entryPath: string): Promise<HmrChildController> {
  const child = spawn(
    nodeBin,
    ["--expose-gc", "--conditions=browser", "./scripts/run-vite-dev-app.mjs", relative(projectRoot, entryPath)],
    {
      cwd: projectRoot,
      env: {
        ...process.env,
        NODE_ENV: "development",
      },
      stdio: ["pipe", "pipe", "pipe"],
    },
  )

  return new HmrChildController(child)
}

describe("vite HMR memory e2e", () => {
  let activeChild: HmrChildController | null = null

  afterEach(async () => {
    await activeChild?.close()
    activeChild = null
  })

  testIfNativeSupported(
    "replaces mounted app handle when root app module updates",
    async () => {
      const fixture = createFixture()

      try {
        const child = await spawnHmrChild(fixture.entryPath)
        activeChild = child

        const baseline = await child.waitForProbe("ready", 15_000)
        expect(baseline.texts).toContain("app version 0")
        expect(baseline.texts).toContain("hmr version 0")

        writeFileSync(fixture.appPath, createAppSource("1"))

        const afterAppUpdate = await child.waitForRenderedText("app version 1", "after-app-hmr")
        expect(afterAppUpdate.texts).toContain("hmr version 0")
        expect(afterAppUpdate.nodeKinds).toEqual(baseline.nodeKinds)
      } finally {
        await activeChild?.close()
        activeChild = null
        rmSync(fixture.dir, { force: true, recursive: true })
      }
    },
    60_000,
  )

  testIfNativeSupported(
    "keeps native tree shape stable while exposing memory probes across repeated HMR updates",
    async () => {
      const fixture = createFixture()

      try {
        const child = await spawnHmrChild(fixture.entryPath)
        activeChild = child

        const baseline = await child.waitForProbe("ready", 15_000)
        expect(baseline.texts).toContain("hmr version 0")
        const baselineHeap = await child.requestHeapSnapshot("ready")

        const observed: ProbeMessage[] = [baseline]

        for (let version = 1; version <= 3; version += 1) {
          writeFileSync(fixture.viewPath, createViewSource(String(version)))
          const probe = await child.waitForRenderedText(`hmr version ${version}`, `after-hmr-${version}`)
          observed.push(probe)
        }

        await sleep(250)
        const settled = await child.requestProbe("settled")
        observed.push(settled)

        await sleep(1_500)
        const afterIdle = await child.requestProbe("after-idle")
        observed.push(afterIdle)
        const afterIdleHeap = await child.requestHeapSnapshot("after-idle")

        for (const probe of observed.slice(1)) {
          expect(probe.nodeCount).toBe(baseline.nodeCount)
          expect(probe.nodeKinds).toEqual(baseline.nodeKinds)
        }

        expect([...new Set(afterIdle.fixtureState.loads)]).toEqual(["0", "1", "2", "3"])
        expect(afterIdle.fixtureState.loads.at(-1)).toBe("3")
        expect(afterIdle.fixtureState.disposals).toEqual(["0", "1", "2"])

        const versionMarkers = ["0", "1", "2", "3"].map((version) => `hmr version ${version}`)
        const baselineHeapMarkers = readMarkerPresence(baselineHeap.path, versionMarkers)
        const afterIdleHeapMarkers = readMarkerPresence(afterIdleHeap.path, versionMarkers)

        const summary = observed.map((probe) => ({
          disposals: probe.fixtureState.disposals,
          environmentIds: probe.viteState.environmentIds,
          label: probe.label,
          loads: probe.fixtureState.loads,
          nodeCount: probe.nodeCount,
          oldSpace: probe.heapSpaces.old_space ?? 0,
          rssAfterGc: probe.memoryAfterGc.rss,
          runnerIds: probe.viteState.runnerIds,
          heapUsedAfterGc: probe.memoryAfterGc.heapUsed,
          texts: probe.texts,
        }))

        console.log("HMR_MEMORY_SUMMARY", JSON.stringify(summary))
        console.log(
          "HMR_HEAP_MARKERS",
          JSON.stringify({
            afterIdle: afterIdleHeapMarkers,
            ready: baselineHeapMarkers,
          }),
        )
      } finally {
        await activeChild?.close()
        activeChild = null
        rmSync(fixture.dir, { force: true, recursive: true })
      }
    },
    60_000,
  )
})
