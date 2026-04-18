import { createSignal } from "solid-js"
import { describe, expect, it } from "vitest"

import type { QtApp, QtHostEvent } from "@qt-solid/core"
import { eventExportIds } from "@qt-solid/core-widgets/widget-library"
import { Button, Column, Input, Label, Text, createApp, createWindow, el } from "@qt-solid/solid"
import { allFakeNodes, fakeNodeByKind, FakeQtApp } from "./mocking/fake-qt"

describe("createApp", () => {
  it("mounts Solid component tree into Qt binding and disposes cleanly", async () => {
    const app = new FakeQtApp()
    const [count] = createSignal(0)

    const mainWindow = createWindow(
      {
        title: "qt-solid",
        width: 360,
        height: 220,
      },
      () =>
        el(Column, {
          gap: 12,
          padding: 16,
          children: [
            el(Text, {
              get children() {
                return `count: ${count()}`
              },
            }),
            el(Label, {
              children: "status",
              minWidth: 90,
              minHeight: 24,
              flexGrow: 2,
              flexShrink: 0,
            }),
            el(Button, { children: "+" }),
            el(Input, { placeholder: "name", minWidth: 140, flexGrow: 1 }),
          ],
        }),
    )

    const mounted = createApp(mainWindow).mount(app as unknown as QtApp)

    await Promise.resolve()

    const nodes = allFakeNodes(app)
    const windowNode = nodes.find((node) => node.kind === "window")
    const viewNode = nodes.find((node) => node.kind === "view")
    const textNode = nodes.find((node) => node.kind === "text")
    const labelNode = nodes.find((node) => node.kind === "label")
    const buttonNode = nodes.find((node) => node.kind === "button")
    const inputNode = nodes.find((node) => node.kind === "input")

    expect(windowNode?.title).toBe("qt-solid")
    expect(windowNode?.width).toBe(360)
    expect(windowNode?.height).toBe(220)
    expect(viewNode?.flexDirection).toBe("column")
    expect(viewNode?.gap).toBe(12)
    expect(viewNode?.padding).toBe(16)
    expect(textNode?.text).toBe("count: 0")
    expect(labelNode?.text).toBe("status")
    expect(labelNode?.minWidth).toBe(90)
    expect(labelNode?.minHeight).toBe(24)
    expect(labelNode?.flexGrow).toBe(2)
    expect(labelNode?.flexShrink).toBe(0)
    expect(buttonNode?.text).toBe("+")
    expect(inputNode?.placeholder).toBe("name")
    expect(inputNode?.minWidth).toBe(140)
    expect(inputNode?.flexGrow).toBe(1)

    mounted.dispose()
    await Promise.resolve()

    expect(windowNode?.destroyed).toBe(true)
    expect(viewNode?.destroyed).toBe(true)
    expect(textNode?.destroyed).toBe(true)
    expect(labelNode?.destroyed).toBe(true)
    expect(buttonNode?.destroyed).toBe(true)
    expect(inputNode?.destroyed).toBe(true)
  })

  it("routes native listener events through attachNativeEvents", async () => {
    const app = new FakeQtApp()
    let handleNativeEvent: (event: QtHostEvent) => void = () => {}
    let clicks = 0

    const mainWindow = createWindow(
      {
        title: "events",
      },
      () => el(Button, { children: "+", onClicked: () => clicks++ }),
    )

    const mounted = createApp(mainWindow).mount(app as unknown as QtApp, {
      attachNativeEvents(handler) {
        handleNativeEvent = handler
      },
    })

    await Promise.resolve()

    const buttonNode = fakeNodeByKind(app, "button")
    expect(buttonNode).toBeDefined()

    handleNativeEvent({
      type: "listener",
      nodeId: buttonNode!.id,
      listenerId: eventExportIds.onClicked,
    } as QtHostEvent)

    expect(clicks).toBe(1)

    mounted.dispose()
  })

  it("defaults last window close request to app shutdown", async () => {
    const app = new FakeQtApp()
    let handleNativeEvent: (event: QtHostEvent) => void = () => {}
    let shutdowns = 0

    app.shutdown = () => {
      shutdowns += 1
    }

    const mainWindow = createWindow(
      {
        title: "close-window",
      },
      () => el(Text, { children: "bye" }),
    )

    createApp(mainWindow).mount(app as unknown as QtApp, {
      attachNativeEvents(handler) {
        handleNativeEvent = handler
      },
    })

    await Promise.resolve()

    const windowNode = fakeNodeByKind(app, "window")
    expect(windowNode).toBeDefined()

    handleNativeEvent({
      type: "listener",
      nodeId: windowNode!.id,
      listenerId: eventExportIds.onCloseRequested,
    } as QtHostEvent)

    await Promise.resolve()
    await Promise.resolve()

    expect(shutdowns).toBe(1)
    expect(windowNode?.destroyed).toBe(true)
  })

  it("runs onWindowAllClosed hook without implicit quit override", async () => {
    const app = new FakeQtApp()
    let shutdowns = 0
    let closed = 0

    app.shutdown = () => {
      shutdowns += 1
    }

    const mainWindow = createWindow(
      {
        title: "hook-window",
      },
      () => el(Text, { children: "hook" }),
    )

    const mounted = createApp({
      render: () => mainWindow.render(),
      onWindowAllClosed() {
        closed += 1
      },
    }).mount(app as unknown as QtApp)

    await Promise.resolve()

    mainWindow.dispose()
    await Promise.resolve()
    await Promise.resolve()

    expect(closed).toBe(1)
    expect(shutdowns).toBe(0)

    mounted.dispose()
  })

  it("reopens last window on app activate when hook keeps app alive", async () => {
    const app = new FakeQtApp()
    let handleNativeEvent: (event: QtHostEvent) => void = () => {}
    let activations = 0

    const mainWindow = createWindow(
      {
        title: "reactivate-window",
      },
      () => el(Text, { children: "back" }),
    )

    const mounted = createApp({
      render: () => mainWindow.render(),
      onWindowAllClosed() {},
      onActivate() {
        activations += 1
        mainWindow.open()
      },
    }).mount(app as unknown as QtApp, {
      attachNativeEvents(handler) {
        handleNativeEvent = handler
      },
    })

    await Promise.resolve()

    const firstWindow = fakeNodeByKind(app, "window")
    expect(firstWindow?.title).toBe("reactivate-window")

    mainWindow.dispose()
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()

    const hiddenWindow = allFakeNodes(app).find(
      (node) => node.kind === "window" && node.title === "reactivate-window" && !node.destroyed,
    )
    expect(hiddenWindow?.visible).toBe(false)

    handleNativeEvent({
      type: "app",
      name: "activate",
    } as QtHostEvent)

    await Promise.resolve()
    await Promise.resolve()

    const reopenedWindow = allFakeNodes(app).find(
      (node) => node.kind === "window" && node.title === "reactivate-window" && !node.destroyed,
    )

    expect(activations).toBe(1)
    expect(reopenedWindow?.visible).toBe(true)

    mounted.dispose()
  })
})
