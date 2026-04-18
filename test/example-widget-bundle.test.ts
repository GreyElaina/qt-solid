import { describe, expect } from "vitest"

import {
  expectCleanExit,
  stripAnsi,
  parseSnapshot,
  runBundledNodeScript,
  testIfNativeSupported,
} from "./mocking/native-run"

describe("example open-world widget library bundle", () => {
  testIfNativeSupported("bundled runtime resolves external widget library by intrinsic name", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-example-widget-bundle",
      entryExtension: ".ts",
      widgetLibraries: ["@qt-solid/core-widgets", "@qt-solid/example-widgets"],
      entrySource: [
        "import type { QtApp } from '@qt-solid/core'",
        "import '@qt-solid/solid'",
        "import { createNativeRendererBinding } from './packages/solid/src/runtime/native-renderer-binding.ts'",
        "",
        "export async function run(app: QtApp) {",
        "  const binding = createNativeRendererBinding(app)",
        "  const window = binding.createElement('window')",
        "  const view = binding.createElement('view')",
        "  const banner = binding.createElement('banner')",
        "  binding.patchProp(window, 'title', undefined, 'Example bundle')",
        "  binding.patchProp(window, 'width', undefined, 320)",
        "  binding.patchProp(window, 'height', undefined, 160)",
        "  binding.patchProp(view, 'direction', undefined, 'column')",
        "  binding.patchProp(view, 'gap', undefined, 6)",
        "  binding.patchProp(view, 'padding', undefined, 4)",
        "  binding.patchProp(banner, 'text', undefined, 'Bundled banner')",
        "  binding.patchProp(banner, 'minWidth', undefined, 180)",
        "  binding.patchProp(banner, 'minHeight', undefined, 32)",
        "  binding.patchProp(banner, 'pointSize', undefined, 18)",
        "  binding.patchProp(banner, 'italic', undefined, true)",
        "  binding.insertChild(app.root, window)",
        "  binding.insertChild(window, view)",
        "  binding.insertChild(view, banner)",
        "  await Promise.resolve()",
        "  console.log('SNAPSHOT', JSON.stringify(app.debugSnapshot()))",
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
        minWidth?: number
        minHeight?: number
        gap?: number
        padding?: number
        flexDirection?: string
        value?: number
      }>
    }>(result.stdout)

    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "window" &&
          node.title === "Example bundle" &&
          node.width === 320 &&
          node.height === 160,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "view" &&
          node.flexDirection === "column" &&
          node.gap === 6 &&
          node.padding === 4,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "label" &&
          node.text === "Bundled banner" &&
          node.minWidth === 180 &&
          node.minHeight === 32,
      ),
    ).toBe(true)
  })

  testIfNativeSupported("bundled runtime imports Banner component from external widget package", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-example-widget-component-bundle",
      entryExtension: ".tsx",
      widgetLibraries: ["@qt-solid/core-widgets", "@qt-solid/example-widgets"],
      entrySource: [
        "import type { QtApp } from '@qt-solid/core'",
        "import { createApp, createWindow } from '@qt-solid/solid'",
        "import { Banner } from '@qt-solid/example-widgets'",
        "",
        "export async function run(app: QtApp) {",
        "  const mounted = createApp(",
        "    createWindow(",
        "      { title: 'Example component bundle', width: 320, height: 160 },",
        "      () => (",
        "        <view direction='column' gap={6} padding={4}>",
        "          <Banner",
        "            text='Bundled banner component'",
        "            minWidth={180}",
        "            minHeight={32}",
        "            pointSize={18}",
        "            italic={true}",
        "          />",
        "        </view>",
        "      ),",
        "    ),",
        "  ).mount(app)",
        "  await Promise.resolve()",
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
        minWidth?: number
        minHeight?: number
        gap?: number
        padding?: number
        flexDirection?: string
      }>
    }>(result.stdout)

    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "window" &&
          node.title === "Example component bundle" &&
          node.width === 320 &&
          node.height === 160,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "view" &&
          node.flexDirection === "column" &&
          node.gap === 6 &&
          node.padding === 4,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "label" &&
          node.text === "Bundled banner component" &&
          node.minWidth === 180 &&
          node.minHeight === 32,
      ),
    ).toBe(true)
  })

  testIfNativeSupported("spin triangle example yields to JS timers while compositor animation runs", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-example-spin-triangle-app",
      entryExtension: ".ts",
      widgetLibraries: ["@qt-solid/core-widgets", "@qt-solid/example-widgets"],
      entrySource: [
        "import type { QtApp } from '@qt-solid/core'",
        "import { __qtSolidDebugCaptureWindowFrame } from '@qt-solid/core'",
        "import { createSpinTriangleApp } from './examples/spin-triangle/spin-triangle-app.tsx'",
        "",
        "export async function run(app: QtApp) {",
        "  const mounted = createSpinTriangleApp().mount(app)",
        "  await new Promise((resolve) => setTimeout(resolve, 120))",
        "  const snapshot = app.debugSnapshot()",
        "  const windowNode = snapshot.nodes.find((node) => node.kind === 'window' && node.title === 'spin_triangle')",
        "  if (!windowNode) {",
        "    throw new Error('missing spin_triangle window node')",
        "  }",
        "  const frameBefore = __qtSolidDebugCaptureWindowFrame(windowNode.id)",
        "  await new Promise((resolve) => setTimeout(resolve, 120))",
        "  const frameAfter = __qtSolidDebugCaptureWindowFrame(windowNode.id)",
        "  console.log('SPIN_FRAME', JSON.stringify({",
        "    beforeSeq: frameBefore.frameSeq,",
        "    afterSeq: frameAfter.frameSeq,",
        "    grouping: frameAfter.grouping,",
        "    partCount: frameAfter.parts.length,",
        "  }))",
        "  mounted.dispose()",
        "}",
      ].join("\n"),
    })

    expectCleanExit(result)

    const stdout = stripAnsi(result.stdout)
    const match = stdout.match(/SPIN_FRAME (\{.*\})/)
    expect(match?.[1]).toBeDefined()
    const frame = JSON.parse(match?.[1] ?? "{}") as {
      beforeSeq: number
      afterSeq: number
      grouping: string
      partCount: number
    }

    expect(frame.afterSeq).toBeGreaterThan(frame.beforeSeq)
    expect(frame.grouping).toBe("segmented")
    expect(frame.partCount).toBeGreaterThan(0)
  })
})
