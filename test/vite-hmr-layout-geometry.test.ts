import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { join, relative } from "node:path"
import { createInterface } from "node:readline"

import { afterEach, describe, expect } from "vitest"

import { nodeBin, projectRoot, stripAnsi, testIfNativeSupported } from "./mocking/native-run"

interface QtBounds {
  height: number
  screenX: number
  screenY: number
  visible: boolean
  width: number
}

interface LayoutProbeMessage {
  bounds: {
    button?: QtBounds
    input?: QtBounds
    window?: QtBounds
  }
  label: string
  texts: string[]
  type: "layout-probe"
}

function sleep(delayMs: number) {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, delayMs)
  })
}

function createViewSource(version: string, padding: number): string {
  return [
    "import type { Component } from 'solid-js'",
    "import { Button, Column, Group, Input, Row, Text } from '@qt-solid/solid'",
    "",
    "export const CalculatorView: Component = () => {",
    "  return (",
    `    <Column gap={12} padding={${String(padding)}}>`,
    "      <Group title='Display'>",
    "        <Column gap={8} padding={8}>",
    `          <Text>version ${version}</Text>`,
    "          <Text>ready</Text>",
    "          <Input text='0' placeholder='0' />",
    "        </Column>",
    "      </Group>",
    "      <Group title='Keypad'>",
    "        <Column gap={8} padding={8}>",
    "          <Row gap={8}>",
    "            <Button>=</Button>",
    "          </Row>",
    "        </Column>",
    "      </Group>",
    "    </Column>",
    "  )",
    "}",
  ].join("\n")
}

function createAppSource(): string {
  return [
    "/// <reference types='vite/client' />",
    "",
    "import { acceptQtSolidDevAppHmr } from '@qt-solid/solid/hmr'",
    "import { createApp, createWindow } from '@qt-solid/solid'",
    "",
    "import { CalculatorView } from './view.tsx'",
    "",
    "const app = createApp(() => {",
    "  const mainWindow = createWindow(",
    "    {",
    "      title: 'hmr-layout-geometry',",
    "      width: 360,",
    "      height: 420,",
    "    },",
    "    () => <CalculatorView />,",
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
    "import { __qtSolidDebugGetNodeBounds } from '@qt-solid/core'",
    "import { createInterface } from 'node:readline'",
    "",
    "interface QtBounds {",
    "  visible: boolean",
    "  screenX: number",
    "  screenY: number",
    "  width: number",
    "  height: number",
    "}",
    "",
    "interface QtDebugNodeSnapshot {",
    "  id: number",
    "  kind: string",
    "  placeholder?: string | null",
    "  text?: string | null",
    "  title?: string | null",
    "}",
    "",
    "interface QtDebugSnapshot {",
    "  nodes: QtDebugNodeSnapshot[]",
    "}",
    "",
    "function emit(message: unknown): void {",
    "  process.stdout.write(`${JSON.stringify(message)}\\n`)",
    "}",
    "",
    "function currentSnapshot(): QtDebugSnapshot | null {",
    "  const state = (globalThis as typeof globalThis & {",
    "    __qtSolidDevSession__?: {",
    "      app?: {",
    "        debugSnapshot(): QtDebugSnapshot",
    "      }",
    "    }",
    "  }).__qtSolidDevSession__",
    "",
    "  try {",
    "    return state?.app?.debugSnapshot() ?? null",
    "  } catch {",
    "    return null",
    "  }",
    "}",
    "",
    "function boundsFor(nodeId?: number): QtBounds | undefined {",
    "  if (nodeId == null) {",
    "    return undefined",
    "  }",
    "",
    "  return __qtSolidDebugGetNodeBounds(nodeId)",
    "}",
    "",
    "function collectProbe(label: string): void {",
    "  const snapshot = currentSnapshot()",
    "  const nodes = snapshot?.nodes ?? []",
    "  const windowNode = nodes.find((node) => node.kind === 'window' && node.title === 'hmr-layout-geometry')",
    "  const inputNode = nodes.find((node) => node.kind === 'input' && node.placeholder === '0')",
    "  const buttonNode = nodes.find((node) => node.kind === 'button' && node.text === '=')",
    "  const texts = nodes",
    "    .filter((node): node is QtDebugNodeSnapshot & { text: string } => node.kind === 'text' && typeof node.text === 'string')",
    "    .map((node) => node.text)",
    "",
    "  emit({",
    "    type: 'layout-probe',",
    "    label,",
    "    texts,",
    "    bounds: {",
    "      window: boundsFor(windowNode?.id),",
    "      input: boundsFor(inputNode?.id),",
    "      button: boundsFor(buttonNode?.id),",
    "    },",
    "  })",
    "}",
    "",
    "async function emitReadyProbe(): Promise<void> {",
    "  for (let attempt = 0; attempt < 80; attempt += 1) {",
    "    const snapshot = currentSnapshot()",
    "    if (snapshot && snapshot.nodes.some((node) => node.kind === 'window')) {",
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
    "startProbeServer()",
  ].join("\n")
}

function createFixture() {
  const dir = mkdtempSync(join(projectRoot, ".tmp-vite-hmr-layout-"))
  const entryPath = join(dir, "dev-entry.tsx")
  const viewPath = join(dir, "view.tsx")

  writeFileSync(viewPath, createViewSource("0", 16))
  writeFileSync(join(dir, "app.tsx"), createAppSource())
  writeFileSync(join(dir, "probe.ts"), createProbeSource())
  writeFileSync(entryPath, createDevEntrySource())

  return { dir, entryPath, viewPath }
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

class GeometryChildController {
  private readonly messages: LayoutProbeMessage[] = []
  private readonly stderrLines: string[] = []
  private readonly stdoutLines: string[] = []
  private readonly waiters = new Set<{
    label: string
    reject: (error: Error) => void
    resolve: (message: LayoutProbeMessage) => void
    timer: ReturnType<typeof setTimeout>
  }>()

  constructor(private readonly child: ChildProcessWithoutNullStreams) {
    const stdout = createInterface({ input: child.stdout })
    const stderr = createInterface({ input: child.stderr })

    stdout.on("line", (line) => {
      this.stdoutLines.push(line)

      let message: LayoutProbeMessage
      try {
        message = JSON.parse(line) as LayoutProbeMessage
      } catch {
        return
      }

      if (message.type !== "layout-probe") {
        return
      }

      for (const waiter of this.waiters) {
        if (waiter.label !== message.label) {
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
        `geometry HMR child exited early (code=${String(code)} signal=${String(signal)})\n${formatOutput(this.stdoutLines, this.stderrLines)}`,
      )

      for (const waiter of this.waiters) {
        clearTimeout(waiter.timer)
        waiter.reject(error)
      }
      this.waiters.clear()
    })
  }

  async waitForProbe(label: string, timeoutMs = 10_000): Promise<LayoutProbeMessage> {
    const existing = this.messages.find((message) => message.label === label)
    if (existing) {
      return existing
    }

    return await new Promise<LayoutProbeMessage>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiters.delete(waiter)
        reject(new Error(`Timed out waiting for probe ${label}\n${formatOutput(this.stdoutLines, this.stderrLines)}`))
      }, timeoutMs)

      const waiter = { label, reject, resolve, timer }
      this.waiters.add(waiter)
    })
  }

  async requestProbe(label: string, timeoutMs = 10_000): Promise<LayoutProbeMessage> {
    this.child.stdin.write(`${JSON.stringify({ type: "probe", label })}\n`)
    return await this.waitForProbe(label, timeoutMs)
  }

  async waitForRenderedText(text: string, labelPrefix: string, attempts = 40): Promise<LayoutProbeMessage> {
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

async function spawnGeometryChild(entryPath: string): Promise<GeometryChildController> {
  const child = spawn(
    nodeBin,
    ["--conditions=browser", "./scripts/run-vite-dev-app.mjs", relative(projectRoot, entryPath)],
    {
      cwd: projectRoot,
      env: {
        ...process.env,
        NODE_ENV: "development",
      },
      stdio: ["pipe", "pipe", "pipe"],
    },
  )

  return new GeometryChildController(child)
}

describe("vite HMR layout geometry e2e", () => {
  let activeChild: GeometryChildController | null = null

  afterEach(async () => {
    await activeChild?.close()
    activeChild = null
  })

  testIfNativeSupported("keeps window bounds stable when a view-only padding edit hot updates", async () => {
    const fixture = createFixture()

    try {
      const child = await spawnGeometryChild(fixture.entryPath)
      activeChild = child

      const baseline = await child.waitForProbe("ready", 15_000)
      expect(baseline.texts).toContain("version 0")
      expect(baseline.bounds.window).toBeDefined()
      expect(baseline.bounds.input).toBeDefined()

      writeFileSync(fixture.viewPath, createViewSource("1", 32))

      await child.waitForRenderedText("version 1", "after-layout-hmr")
      await sleep(150)
      const afterUpdate = await child.requestProbe("after-layout-hmr-settled")

      expect(afterUpdate.bounds.window?.width).toBe(baseline.bounds.window?.width)
      expect(afterUpdate.bounds.window?.height).toBe(baseline.bounds.window?.height)

      console.log(
        "HMR_LAYOUT_GEOMETRY",
        JSON.stringify({
          afterUpdate: afterUpdate.bounds,
          baseline: baseline.bounds,
        }),
      )
    } finally {
      await activeChild?.close()
      activeChild = null
      rmSync(fixture.dir, { force: true, recursive: true })
    }
  }, 60_000)
})
