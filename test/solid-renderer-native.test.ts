import { describe, expect } from "vitest"

import {
  expectCleanExit,
  parseSnapshot,
  runBundledNodeScript,
  stripAnsi,
  testIfNativeSupported,
} from "./mocking/native-run"

function parseMatchedJson<T>(value: string, pattern: RegExp): T {
  const match = value.match(pattern)
  expect(match?.[1]).toBeDefined()
  const payload = match?.[1]
  if (!payload) {
    throw new Error(`missing payload for ${pattern}`)
  }
  return JSON.parse(payload) as T
}

describe("native Solid renderer", () => {
  testIfNativeSupported("createApp drives native snapshot through Solid updates", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-solid-renderer-entry",
      entryExtension: ".ts",
      entrySource: [
        "import { createSignal } from 'solid-js'",
        "import { captureWindowFrame } from '@qt-solid/core/native'",
        "import type { QtApp } from '@qt-solid/core'",
        "import { Button, Column, Input, Label, Text, createApp, createWindow, el } from '@qt-solid/solid'",
        "",
        "export async function run(app: QtApp) {",
        "  const [count, setCount] = createSignal(0)",
        "",
        "  const mainWindow = createWindow(",
        "    {",
        "      title: 'solid-native',",
        "      width: 420,",
        "      height: 240,",
        "    },",
        "    () =>",
        "      el(Column, {",
        "        gap: 10,",
        "        padding: 14,",
        "        children: [",
        "          el(Text, { get children() { return 'count: ' + count() } }),",
        "          el(Label, { children: 'status', minWidth: 90, minHeight: 24, flexGrow: 2, flexShrink: 0 }),",
        "          el(Button, { children: '+' }),",
        "          el(Input, { placeholder: 'name', minWidth: 140, flexGrow: 1 }),",
        "        ],",
        "      }),",
        "  )",
        "",
        "  const mounted = createApp(mainWindow).mount(app)",
        "  await Promise.resolve()",
        "  setCount(2)",
        "  await Promise.resolve()",
        "  await Promise.resolve()",
        "",
        "  console.log('SNAPSHOT', JSON.stringify(app.debugSnapshot()))",
        "  mounted.dispose()",
        "}",
      ].join("\n"),
    })

    expectCleanExit(result)

    const snapshot = parseSnapshot<{
      nodes: Array<{
        kind: string
        title?: string
        width?: number
        height?: number
        text?: string
        placeholder?: string
        flexDirection?: string
        gap?: number
        padding?: number
        minWidth?: number
        minHeight?: number
        flexGrow?: number
        flexShrink?: number
      }>
    }>(result.stdout)

    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "window" &&
          node.title === "solid-native" &&
          node.width === 420 &&
          node.height === 240,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "view" &&
          node.flexDirection === "column" &&
          node.gap === 10 &&
          node.padding === 14,
      ),
    ).toBe(true)
    expect(snapshot.nodes.some((node) => node.kind === "text" && node.text === "count: 2")).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "label" &&
          node.text === "status" &&
          node.minWidth === 90 &&
          node.minHeight === 24 &&
          node.flexGrow === 2 &&
          node.flexShrink === 0,
      ),
    ).toBe(true)
    expect(snapshot.nodes.some((node) => node.kind === "button" && node.text === "+")).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "input" &&
          node.placeholder === "name" &&
          node.minWidth === 140 &&
          node.flexGrow === 1,
      ),
    ).toBe(true)
  })

  testIfNativeSupported("last window close path triggers app quit when hook calls quit", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-solid-window-close-exit",
      entryExtension: ".tsx",
      entrySource: [
        "import type { QtApp } from '@qt-solid/core'",
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "export async function run(app: QtApp) {",
        "  const mainWindow = createWindow(",
        "    { title: 'close-exit', width: 280, height: 160 },",
        "    () => <Text>close me</Text>,",
        "  )",
        "",
        "  const mounted = createApp(() => ({",
        "    render: () => mainWindow.render(),",
        "    onWindowAllClosed({ quit }) {",
        "      console.log('WINDOWS_CLOSED')",
        "      quit()",
        "    },",
        "  })).mount(app)",
        "",
        "  await Promise.resolve()",
        "  await Promise.resolve()",
        "  await new Promise((resolve) => setTimeout(resolve, 20))",
        "",
        "  mainWindow.dispose()",
        "  await new Promise((resolve) => setTimeout(resolve, 50))",
        "",
        "  try {",
        "    void app.root",
        "    console.log('APP_STILL_RUNNING')",
        "  } catch {",
        "    console.log('APP_SHUT_DOWN')",
        "  }",
        "",
        "  mounted.dispose()",
        "}",
      ].join("\n"),
    })

    expectCleanExit(result)
    const stdout = stripAnsi(result.stdout)
    expect(stdout).toContain("WINDOWS_CLOSED")
    expect(stdout).toContain("APP_SHUT_DOWN")
    expect(stdout).not.toContain("APP_STILL_RUNNING")
  })

  testIfNativeSupported("createWindow handle exposes frame state, repaint, and capture", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-solid-window-handle-frame",
      entryExtension: ".ts",
      entrySource: [
        "import type { QtApp } from '@qt-solid/core'",
        "import { captureWindowFrame } from '@qt-solid/core/native'",
        "import { Text, createApp, createWindow } from '@qt-solid/solid'",
        "",
        "export async function run(app: QtApp) {",
        "  const mainWindow = createWindow(",
        "    { title: 'window-handle-frame', width: 280, height: 160 },",
        "    () => Text({ children: 'frame handle' }),",
        "  )",
        "",
        "  const mounted = createApp(mainWindow).mount(app)",
        "  await new Promise((resolve) => setTimeout(resolve, 80))",
        "  const before = mainWindow.frameState()",
        "  mainWindow.requestNextFrame()",
        "  await new Promise((resolve) => setTimeout(resolve, 80))",
        "  const after = mainWindow.frameState()",
        "  mainWindow.requestRepaint()",
        "  await new Promise((resolve) => setTimeout(resolve, 40))",
        "  const capture = mainWindow.capture()",
        "  const snapshot = app.debugSnapshot()",
        "  const windowNode = snapshot.nodes.find((node) => node.kind === 'window' && node.title === 'window-handle-frame')",
        "  if (!windowNode) {",
        "    throw new Error('missing mounted window node for debug frame capture')",
        "  }",
        "  const segmentedFrame = captureWindowFrame(windowNode.id)",
        "  console.log('FRAME_BEFORE', JSON.stringify(before))",
        "  console.log('FRAME_AFTER', JSON.stringify(after))",
        "  console.log('CAPTURE', JSON.stringify({",
        "    format: capture.format,",
        "    widthPx: capture.widthPx,",
        "    heightPx: capture.heightPx,",
        "    stride: capture.stride,",
        "    scaleFactor: capture.scaleFactor,",
        "    byteLength: capture.bytes.length,",
        "  }))",
        "  console.log('SEGMENTED_FRAME', JSON.stringify(segmentedFrame))",
        "  mounted.dispose()",
        "  app.shutdown()",
        "}",
      ].join("\n"),
    })

    expectCleanExit(result)
    const stdout = stripAnsi(result.stdout)
    const frameBefore = parseMatchedJson<{
      seq: number
      elapsedMs: number
      deltaMs: number
    }>(stdout, /FRAME_BEFORE (\{.*\})/)
    const frameAfter = parseMatchedJson<{
      seq: number
      elapsedMs: number
      deltaMs: number
    }>(stdout, /FRAME_AFTER (\{.*\})/)
    const capture = parseMatchedJson<{
      format: string
      widthPx: number
      heightPx: number
      stride: number
      scaleFactor: number
      byteLength: number
    }>(stdout, /CAPTURE (\{.*\})/)
    const segmentedFrame = parseMatchedJson<{
      grouping: string
      frameSeq: number
      parts: Array<{ nodeId: number; byteLength: number }>
    }>(stdout, /SEGMENTED_FRAME (\{.*\})/)

    expect(frameAfter.seq).toBeGreaterThan(frameBefore.seq)
    expect(frameAfter.elapsedMs).toBeGreaterThanOrEqual(frameBefore.elapsedMs)
    expect(capture.format).toBe("argb32-premultiplied")
    expect(capture.widthPx).toBeGreaterThan(0)
    expect(capture.heightPx).toBeGreaterThan(0)
    expect(capture.scaleFactor).toBeGreaterThan(0)
    expect(capture.byteLength).toBe(capture.heightPx * capture.stride)
    expect(segmentedFrame.grouping).toBe("segmented")
    expect(segmentedFrame.frameSeq).toBeGreaterThanOrEqual(frameAfter.seq)
    expect(segmentedFrame.parts.length).toBeGreaterThanOrEqual(1)
    expect(segmentedFrame.parts.every((item) => item.byteLength > 0)).toBe(true)
  })
})
