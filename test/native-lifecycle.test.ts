import { describe, expect } from "vitest"

import {
  coreWidgetsNativeModuleSpecifier,
  expectCleanExit,
  nativeModuleSpecifier,
  parseSnapshot,
  runNodeScript,
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

describe("native Qt lifecycle", () => {
  testIfNativeSupported(
    "Node host lifecycle and debug snapshot stay stable",
    () => {
      const shutdownResult = runNodeScript([
        `import { QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"))
      expectCleanExit(shutdownResult)

      const timerResult = runNodeScript([
        `import { QtApp, __qtSolidDebugScheduleTimerEvent } from ${JSON.stringify(nativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start((event) => {",
        "  console.log('EVENT', JSON.stringify(event))",
        "  if (event.type === 'debug' && event.name === 'qt-timer-bridge') {",
        "    app.shutdown()",
        "    process.exit(0)",
        "  }",
        "})",
        "",
        "__qtSolidDebugScheduleTimerEvent(100, 'qt-timer-bridge')",
      ].join("\n"))
      expectCleanExit(timerResult)
      expect(stripAnsi(timerResult.stdout)).toContain(
        'EVENT {"type":"debug","name":"qt-timer-bridge"}',
      )

      const appEventResult = runNodeScript([
        `import { QtApp, __qtSolidDebugEmitAppEvent } from ${JSON.stringify(nativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start((event) => {",
        "  console.log('EVENT', JSON.stringify(event))",
        "  if (event.type === 'app' && event.name === 'activate') {",
        "    app.shutdown()",
        "    process.exit(0)",
        "  }",
        "})",
        "",
        "__qtSolidDebugEmitAppEvent('activate')",
      ].join("\n"))
      expectCleanExit(appEventResult)
      expect(stripAnsi(appEventResult.stdout)).toContain(
        'EVENT {"type":"app","name":"activate"}',
      )

      const closeResult = runNodeScript([
        `import { QtApp, __qtSolidDebugCloseNode } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const events = []",
        "const app = QtApp.start((event) => {",
        "  events.push(event)",
        "})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "root.insertChild(window.node)",
        "__qtSolidDebugCloseNode(window.node.id)",
        "setTimeout(() => {",
        "  console.log('EVENTS', JSON.stringify(events))",
        "  app.shutdown()",
        "  process.exit(0)",
        "}, 50)",
      ].join("\n"))
      expectCleanExit(closeResult)
      expect(stripAnsi(closeResult.stdout)).toContain('"type":"listener"')

      const highlightResult = runNodeScript([
        `import { QtApp, __qtSolidDebugClearHighlight, __qtSolidDebugGetNodeAtPoint, __qtSolidDebugGetNodeBounds, __qtSolidDebugHighlightNode, __qtSolidDebugSetInspectMode } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtButton, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const button = QtButton.create(app)",
        "root.insertChild(window.node)",
        "window.node.insertChild(button.node)",
        "window.setWidth(320)",
        "window.setHeight(180)",
        "button.setText('inspect me')",
        "console.log('BUTTON', button.node.id)",
        "setTimeout(() => {",
        "  const bounds = __qtSolidDebugGetNodeBounds(button.node.id)",
        "  const centerX = bounds.screenX + Math.floor(bounds.width / 2)",
        "  const centerY = bounds.screenY + Math.floor(bounds.height / 2)",
        "  console.log('BOUNDS', JSON.stringify(bounds))",
        "  console.log('HIT', __qtSolidDebugGetNodeAtPoint(centerX, centerY))",
        "  __qtSolidDebugSetInspectMode(true)",
        "  __qtSolidDebugSetInspectMode(false)",
        "  __qtSolidDebugHighlightNode(button.node.id)",
        "  __qtSolidDebugClearHighlight()",
        "  app.shutdown()",
        "  process.exit(0)",
        "}, 50)",
      ].join("\n"))
      expectCleanExit(highlightResult)
      const boundsMatch = stripAnsi(highlightResult.stdout).match(/BOUNDS (\{.*\})/)
      expect(boundsMatch).not.toBeNull()
      if (!boundsMatch?.[1]) {
        throw new Error("missing debug node bounds payload")
      }
      const bounds = JSON.parse(boundsMatch[1]) as {
        visible: boolean
        screenX: number
        screenY: number
        width: number
        height: number
      }
      expect(bounds.visible).toBe(true)
      expect(bounds.width).toBeGreaterThan(0)
      expect(bounds.height).toBeGreaterThan(0)
      const buttonMatch = stripAnsi(highlightResult.stdout).match(/BUTTON (\d+)/)
      const hitMatch = stripAnsi(highlightResult.stdout).match(/HIT (\d+)/)
      expect(buttonMatch?.[1]).toBeDefined()
      expect(hitMatch?.[1]).toBeDefined()
      expect(Number(hitMatch?.[1])).toBe(Number(buttonMatch?.[1]))

      const frameCaptureResult = runNodeScript([
        `import { QtApp, __qtSolidDebugCaptureWindowFrame } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtText, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const text = QtText.create(app)",
        "root.insertChild(window.node)",
        "window.node.insertChild(text.node)",
        "window.setTitle('frame-capture')",
        "window.setWidth(260)",
        "window.setHeight(140)",
        "text.setText('frame capture')",
        "setTimeout(() => {",
        "  const before = {",
        "    seq: window.getSeq(),",
        "    elapsedMs: window.getElapsedMs(),",
        "    deltaMs: window.getDeltaMs(),",
        "  }",
        "  window.setNextFrameRequested(true)",
        "  window.node.__qtSolidRequestRepaint()",
        "  setTimeout(() => {",
        "    const after = {",
        "      seq: window.getSeq(),",
        "      elapsedMs: window.getElapsedMs(),",
        "      deltaMs: window.getDeltaMs(),",
        "    }",
        "    const capture = window.node.__qtSolidCaptureWidget()",
        "    const frame = __qtSolidDebugCaptureWindowFrame(window.node.id)",
        "    console.log('FRAME_BEFORE', JSON.stringify(before))",
        "    console.log('FRAME_AFTER', JSON.stringify(after))",
        "    console.log('CAPTURE', JSON.stringify({",
        "      format: capture.format,",
        "      widthPx: capture.widthPx,",
        "      heightPx: capture.heightPx,",
        "      stride: capture.stride,",
        "      scaleFactor: capture.scaleFactor,",
        "      byteLength: capture.bytes.length,",
        "    }))",
        "    console.log('WINDOW_FRAME', JSON.stringify(frame))",
        "    app.shutdown()",
        "    process.exit(0)",
        "  }, 80)",
        "}, 80)",
      ].join("\n"))
      expectCleanExit(frameCaptureResult)
      const frameOutput = stripAnsi(frameCaptureResult.stdout)
      const frameBefore = parseMatchedJson<{
        seq: number
        elapsedMs: number
        deltaMs: number
      }>(frameOutput, /FRAME_BEFORE (\{.*\})/)
      const frameAfter = parseMatchedJson<{
        seq: number
        elapsedMs: number
        deltaMs: number
      }>(frameOutput, /FRAME_AFTER (\{.*\})/)
      const capture = parseMatchedJson<{
        format: string
        widthPx: number
        heightPx: number
        stride: number
        scaleFactor: number
        byteLength: number
      }>(frameOutput, /CAPTURE (\{.*\})/)
      const windowFrame = parseMatchedJson<{
        windowId: number
        grouping: string
        frameSeq: number
        elapsedMs: number
        deltaMs: number
        parts: Array<{
          nodeId: number
          x: number
          y: number
          width: number
          height: number
          widthPx: number
          heightPx: number
          stride: number
          scaleFactor: number
          byteLength: number
        }>
      }>(frameOutput, /WINDOW_FRAME (\{.*\})/)
      expect(frameAfter.seq).toBeGreaterThan(frameBefore.seq)
      expect(frameAfter.elapsedMs).toBeGreaterThanOrEqual(frameBefore.elapsedMs)
      expect(capture.format).toBe("argb32-premultiplied")
      expect(capture.widthPx).toBeGreaterThan(0)
      expect(capture.heightPx).toBeGreaterThan(0)
      expect(capture.scaleFactor).toBeGreaterThan(0)
      expect(capture.byteLength).toBe(capture.heightPx * capture.stride)
      expect(windowFrame.windowId).toBeGreaterThan(0)
      expect(windowFrame.grouping).toBe("segmented")
      expect(windowFrame.frameSeq).toBeGreaterThanOrEqual(frameAfter.seq)
      expect(windowFrame.elapsedMs).toBeGreaterThanOrEqual(frameAfter.elapsedMs)
      expect(windowFrame.parts.length).toBeGreaterThanOrEqual(2)
      expect(windowFrame.parts[0]?.x).toBe(0)
      expect(windowFrame.parts[0]?.y).toBe(0)
      expect(windowFrame.parts.every((item) => item.byteLength === item.heightPx * item.stride)).toBe(true)
      expect(windowFrame.parts.some((item) => item.nodeId === windowFrame.windowId)).toBe(true)

      const velloCaptureResult = runNodeScript([
        `import { QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtCanvas, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const canvas = QtCanvas.create(app)",
        "root.insertChild(window.node)",
        "window.node.insertChild(canvas.node)",
        "window.setTitle('vello-window-capture')",
        "window.setWidth(240)",
        "window.setHeight(140)",
        "canvas.setWidth(120)",
        "canvas.setHeight(72)",
        "setTimeout(() => {",
        "  const canvasCapture = canvas.node.__qtSolidCaptureWidget()",
        "  const windowCapture = window.node.__qtSolidCaptureWidget()",
        "  console.log('CANVAS_CAPTURE', JSON.stringify({",
        "    format: canvasCapture.format,",
        "    widthPx: canvasCapture.widthPx,",
        "    heightPx: canvasCapture.heightPx,",
        "    stride: canvasCapture.stride,",
        "    byteLength: canvasCapture.bytes.length,",
        "  }))",
        "  console.log('WINDOW_CAPTURE', JSON.stringify({",
        "    format: windowCapture.format,",
        "    widthPx: windowCapture.widthPx,",
        "    heightPx: windowCapture.heightPx,",
        "    stride: windowCapture.stride,",
        "    byteLength: windowCapture.bytes.length,",
        "  }))",
        "  app.shutdown()",
        "  process.exit(0)",
        "}, 80)",
      ].join("\n"))
      expectCleanExit(velloCaptureResult)
      const velloOutput = stripAnsi(velloCaptureResult.stdout)
      const canvasCapture = parseMatchedJson<{
        format: string
        widthPx: number
        heightPx: number
        stride: number
        byteLength: number
      }>(velloOutput, /CANVAS_CAPTURE (\{.*\})/)
      const windowCapture = parseMatchedJson<{
        format: string
        widthPx: number
        heightPx: number
        stride: number
        byteLength: number
      }>(velloOutput, /WINDOW_CAPTURE (\{.*\})/)
      expect(canvasCapture.format).toBe("rgba8-premultiplied")
      expect(canvasCapture.widthPx).toBeGreaterThan(0)
      expect(canvasCapture.heightPx).toBeGreaterThan(0)
      expect(canvasCapture.byteLength).toBe(canvasCapture.heightPx * canvasCapture.stride)
      expect(windowCapture.format).toBe("argb32-premultiplied")
      expect(windowCapture.widthPx).toBeGreaterThanOrEqual(canvasCapture.widthPx)
      expect(windowCapture.heightPx).toBeGreaterThanOrEqual(canvasCapture.heightPx)
      expect(windowCapture.byteLength).toBe(windowCapture.heightPx * windowCapture.stride)

      const autonomousNativeRepaintResult = runNodeScript([
        `import { QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtCanvas, QtInput, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const canvas = QtCanvas.create(app)",
        "const input = QtInput.create(app)",
        "root.insertChild(window.node)",
        "window.node.insertChild(canvas.node)",
        "window.node.insertChild(input.node)",
        "window.setTitle('native-autonomous-repaint')",
        "window.setWidth(260)",
        "window.setHeight(160)",
        "canvas.setWidth(140)",
        "canvas.setHeight(72)",
        "input.setText('Akashina')",
        "input.setAutoFocus(true)",
        "setTimeout(() => {",
        "  const before = {",
        "    seq: window.getSeq(),",
        "    capture: window.node.__qtSolidCaptureWidget(),",
        "  }",
        "  setTimeout(() => {",
        "    const after = {",
        "      seq: window.getSeq(),",
        "      capture: window.node.__qtSolidCaptureWidget(),",
        "    }",
        "    console.log('AUTONOMOUS_REPAINT', JSON.stringify({",
        "      beforeSeq: before.seq,",
        "      afterSeq: after.seq,",
        "      beforeFormat: before.capture.format,",
        "      afterFormat: after.capture.format,",
        "    }))",
        "    app.shutdown()",
        "    process.exit(0)",
        "  }, 1100)",
        "}, 250)",
      ].join("\n"))
      expectCleanExit(autonomousNativeRepaintResult)
      const autonomous = parseMatchedJson<{
        beforeSeq: number
        afterSeq: number
        beforeFormat: string
        afterFormat: string
      }>(stripAnsi(autonomousNativeRepaintResult.stdout), /AUTONOMOUS_REPAINT (\{.*\})/)
      expect(autonomous.beforeFormat).toBe("argb32-premultiplied")
      expect(autonomous.afterFormat).toBe("argb32-premultiplied")
      expect(autonomous.afterSeq).toBeGreaterThanOrEqual(autonomous.beforeSeq)

      const mountResult = runNodeScript([
        `import { AlignItems, FlexDirection, JustifyContent, QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtLabel, QtView, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const view = QtView.create(app)",
        "const label = QtLabel.create(app)",
        "",
        "root.insertChild(window.node)",
        "window.node.insertChild(view.node)",
        "view.node.insertChild(label.node)",
        "window.setTitle('qt-solid')",
        "window.setWidth(320)",
        "window.setHeight(180)",
        "view.setDirection(FlexDirection.Column)",
        "view.setAlignItems(AlignItems.Stretch)",
        "view.setJustifyContent(JustifyContent.FlexStart)",
        "view.setGap(8)",
        "view.setPadding(12)",
        "label.setText('hello')",
        "",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"))
      expectCleanExit(mountResult)

      const snapshotResult = runNodeScript([
        `import { AlignItems, FlexDirection, JustifyContent, QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtInput, QtView, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const view = QtView.create(app)",
        "const input = QtInput.create(app)",
        "",
        "root.insertChild(window.node)",
        "window.node.insertChild(view.node)",
        "view.node.insertChild(input.node)",
        "window.setTitle('debug-window')",
        "view.setDirection(FlexDirection.Row)",
        "view.setAlignItems(AlignItems.Center)",
        "view.setJustifyContent(JustifyContent.Center)",
        "view.setGap(6)",
        "view.setPadding(10)",
        "input.setPlaceholder('name')",
        "input.setText('Akashina')",
        "input.setMinWidth(140)",
        "input.setMinHeight(28)",
        "input.setGrow(1)",
        "input.setShrink(0)",
        "",
        "console.log('SNAPSHOT', JSON.stringify(app.debugSnapshot()))",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"))
      expectCleanExit(snapshotResult)

      const snapshot = parseSnapshot<{
        hostRuntime: string
        windowHostBackend?: string
        windowHostCapabilities?: {
          backendKind: string
          supportsZeroTimeoutPump: boolean
          supportsExternalWake: boolean
          supportsFdBridge: boolean
        }
        nodes: Array<{
          kind: string
          title?: string
          placeholder?: string
          text?: string
          flexDirection?: string
          alignItems?: string
          justifyContent?: string
          gap?: number
          padding?: number
          minWidth?: number
          minHeight?: number
          flexGrow?: number
          flexShrink?: number
        }>
      }>(snapshotResult.stdout)

      expect(snapshot.hostRuntime).toBe("nodejs")
      expect(
        snapshot.nodes.some(
          (node) =>
            node.kind === "view" &&
            node.flexDirection === "row" &&
            node.alignItems === "center" &&
            node.justifyContent === "center" &&
            node.gap === 6 &&
            node.padding === 10,
        ),
      ).toBe(true)
      expect(
        snapshot.nodes.some((node) => node.kind === "window" && node.title === "debug-window"),
      ).toBe(true)
      expect(
        snapshot.nodes.some(
          (node) =>
            node.kind === "input" &&
            node.placeholder === "name" &&
            node.text === "Akashina" &&
            node.minWidth === 140 &&
            node.minHeight === 28 &&
            node.flexGrow === 1 &&
            node.flexShrink === 0,
        ),
      ).toBe(true)

      const windowHostInfoResult = runNodeScript([
        `import { QtApp, __qtSolidWindowHostInfo } from ${JSON.stringify(nativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "console.log('INFO', JSON.stringify(__qtSolidWindowHostInfo()))",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"))
      expectCleanExit(windowHostInfoResult)
      const windowHostInfoMatch = stripAnsi(windowHostInfoResult.stdout).match(/INFO (\{.*\})/)
      expect(windowHostInfoMatch).not.toBeNull()
      if (!windowHostInfoMatch?.[1]) {
        throw new Error("missing window host info payload")
      }
      const windowHostInfo = JSON.parse(windowHostInfoMatch[1]) as {
        enabled: boolean
        backendName: string
        capabilities: {
          backendKind: string
          supportsZeroTimeoutPump: boolean
          supportsExternalWake: boolean
          supportsFdBridge: boolean
        }
      }
      expect(windowHostInfo.enabled).toBe(true)
      expect(snapshot.windowHostBackend).toBe(windowHostInfo.backendName)
      expect(snapshot.windowHostCapabilities).toEqual(windowHostInfo.capabilities)
      expect(windowHostInfo.backendName).toBe("macos")
      expect(windowHostInfo.capabilities).toEqual({
        backendKind: "macos",
        supportsZeroTimeoutPump: true,
        supportsExternalWake: true,
        supportsFdBridge: true,
      })

      const windowHostShutdownResult = runNodeScript([
        `import { QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start(() => {})",
        "console.log('SNAPSHOT', JSON.stringify(app.debugSnapshot()))",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"))
      expectCleanExit(windowHostShutdownResult)
      const windowHostSnapshot = parseSnapshot<{
        windowHostBackend?: string
        windowHostCapabilities?: {
          backendKind: string
          supportsZeroTimeoutPump: boolean
          supportsExternalWake: boolean
          supportsFdBridge: boolean
        }
      }>(windowHostShutdownResult.stdout)
      expect(windowHostSnapshot.windowHostBackend).toBe("macos")
      expect(windowHostSnapshot.windowHostCapabilities).toEqual({
        backendKind: "macos",
        supportsZeroTimeoutPump: true,
        supportsExternalWake: true,
        supportsFdBridge: true,
      })

      const silentWriteResult = runNodeScript([
        `import { QtApp } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtCheck, QtInput, QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const events = []",
        "const app = QtApp.start((event) => {",
        "  if (event.type === 'listener') {",
        "    events.push(event)",
        "  }",
        "})",
        "",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "const input = QtInput.create(app)",
        "const check = QtCheck.create(app)",
        "",
        "root.insertChild(window.node)",
        "window.node.insertChild(input.node)",
        "window.node.insertChild(check.node)",
        "input.setText('Akashina')",
        "input.setText('Akashina')",
        "check.setChecked(true)",
        "check.setChecked(true)",
        "",
        "console.log('EVENTS', JSON.stringify(events))",
        "app.shutdown()",
        "process.exit(0)",
      ].join("\n"))
      expectCleanExit(silentWriteResult)
      expect(stripAnsi(silentWriteResult.stdout)).toContain("EVENTS []")
    },
    60_000,
  )

  testIfNativeSupported(
    "window close request can immediately shut down app without crashing",
    () => {
      const closeShutdownResult = runNodeScript([
        `import { QtApp, __qtSolidDebugCloseNode } from ${JSON.stringify(nativeModuleSpecifier)}`,
        `import { QtWindow } from ${JSON.stringify(coreWidgetsNativeModuleSpecifier)}`,
        "",
        "const app = QtApp.start((event) => {",
        "  console.log('EVENT', JSON.stringify(event))",
        "  if (event.type === 'listener') {",
        "    app.shutdown()",
        "    process.exit(0)",
        "  }",
        "})",
        "const root = app.root",
        "const window = QtWindow.create(app)",
        "root.insertChild(window.node)",
        "setTimeout(() => {",
        "  console.log('TIMEOUT')",
        "  app.shutdown()",
        "  process.exit(1)",
        "}, 500)",
        "__qtSolidDebugCloseNode(window.node.id)",
      ].join("\n"))

      expectCleanExit(closeShutdownResult)
      expect(stripAnsi(closeShutdownResult.stdout)).toContain('"type":"listener"')
      expect(stripAnsi(closeShutdownResult.stdout)).not.toContain("TIMEOUT")
    },
    20_000,
  )
})
