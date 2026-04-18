import { createSignal } from "solid-js"
import { describe, expect, it } from "vitest"

import type { QtApp } from "@qt-solid/core"
import { rendererInspectorStore } from "../packages/solid/src/devtools/inspector-store"
import { Button, Column, createApp, createWindow, Input, Label, Text } from "../packages/solid/src"
import { allFakeNodes, FakeQtApp } from "./mocking/fake-qt"

describe("createApp TSX", () => {
  it("mounts TSX component tree through createApp and disposes cleanly", async () => {
    const app = new FakeQtApp()
    const [count] = createSignal(0)

    const mainWindow = createWindow(
      {
        title: "tsx-solid",
        width: 480,
        height: 260,
      },
      () => (
        <Column gap={9} padding={11}>
          <Text>{`count: ${count()}`}</Text>
          <Label minWidth={90} minHeight={24} flexGrow={2} flexShrink={0}>
            status
          </Label>
          <Button>+</Button>
          <Input placeholder="name" minWidth={140} flexGrow={1} />
        </Column>
      ),
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

    expect(windowNode?.title).toBe("tsx-solid")
    expect(windowNode?.width).toBe(480)
    expect(windowNode?.height).toBe(260)
    expect(viewNode?.flexDirection).toBe("column")
    expect(viewNode?.gap).toBe(9)
    expect(viewNode?.padding).toBe(11)
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

  it("records component owner metadata on renderer nodes", async () => {
    const app = new FakeQtApp()

    const CounterLabel = () => <Text>count</Text>
    const CounterPanel = () => (
      <Column gap={9} padding={11}>
        <CounterLabel />
        <Button>+</Button>
      </Column>
    )

    const mainWindow = createWindow(
      {
        title: "owner-solid",
        width: 320,
        height: 180,
      },
      () => <CounterPanel />,
    )

    const mounted = createApp(mainWindow).mount(app as unknown as QtApp)

    await Promise.resolve()

    const snapshot = rendererInspectorStore.snapshot()
    const viewNode = snapshot.nodes.find((node) => node.kind === "view")
    const textNode = snapshot.nodes.find((node) => node.kind === "text" && node.props.text === "count")
    const buttonNode = snapshot.nodes.find((node) => node.kind === "button")

    expect(viewNode?.owner?.ownerStack.map((frame) => frame.componentName)).toEqual(["CounterPanel", "Column"])
    expect(textNode?.owner?.ownerStack.map((frame) => frame.componentName)).toEqual([
      "CounterPanel",
      "Column",
      "CounterLabel",
      "Text",
    ])
    expect(buttonNode?.owner?.ownerStack.map((frame) => frame.componentName)).toEqual([
      "CounterPanel",
      "Column",
      "Button",
    ])
    expect(textNode?.owner?.ownerStack.every((frame) => frame.source != null)).toBe(true)

    mounted.dispose()
  })

  it("mounts intrinsic widget body through createWindow without raw window JSX", async () => {
    const app = new FakeQtApp()

    const mainWindow = createWindow(
      {
        title: "intrinsic-solid",
        width: 320,
        height: 180,
      },
      () => (
        <view gap={6} padding={10} direction="column">
          <text text="hello" />
          <button text="+" />
          <input placeholder="name" text="Akashina" />
        </view>
      ),
    )

    const dispose = mainWindow.renderQt(app as unknown as QtApp)

    await Promise.resolve()

    const nodes = allFakeNodes(app)
    const windowNode = nodes.find((node) => node.kind === "window")
    const viewNode = nodes.find((node) => node.kind === "view")
    const textNode = nodes.find((node) => node.kind === "text")
    const buttonNode = nodes.find((node) => node.kind === "button")
    const inputNode = nodes.find((node) => node.kind === "input")

    expect(windowNode?.title).toBe("intrinsic-solid")
    expect(windowNode?.width).toBe(320)
    expect(windowNode?.height).toBe(180)
    expect(viewNode?.flexDirection).toBe("column")
    expect(viewNode?.gap).toBe(6)
    expect(viewNode?.padding).toBe(10)
    expect(textNode?.text).toBe("hello")
    expect(buttonNode?.text).toBe("+")
    expect(inputNode?.placeholder).toBe("name")
    expect(inputNode?.text).toBe("Akashina")

    dispose()
    await Promise.resolve()

    expect(windowNode?.destroyed).toBe(true)
    expect(viewNode?.destroyed).toBe(true)
    expect(textNode?.destroyed).toBe(true)
    expect(buttonNode?.destroyed).toBe(true)
    expect(inputNode?.destroyed).toBe(true)
  })

  it("updates layout props reactively after mount", async () => {
    const app = new FakeQtApp()
    const [gap, setGap] = createSignal<number | undefined>(6)
    const [padding, setPadding] = createSignal<number | undefined>(10)
    const [direction, setDirection] = createSignal<"column" | "row" | undefined>("column")

    const mainWindow = createWindow(
      {
        title: "intrinsic-reactive-layout",
        width: 320,
        height: 180,
      },
      () => (
        <view gap={gap()} padding={padding()} direction={direction()}>
          <text text="hello" />
        </view>
      ),
    )

    const dispose = mainWindow.renderQt(app as unknown as QtApp)

    await Promise.resolve()

    const viewNode = allFakeNodes(app).find((node) => node.kind === "view")
    expect(viewNode?.flexDirection).toBe("column")
    expect(viewNode?.gap).toBe(6)
    expect(viewNode?.padding).toBe(10)

    setGap(14)
    setPadding(18)
    setDirection("row")
    await Promise.resolve()

    expect(viewNode?.flexDirection).toBe("row")
    expect(viewNode?.gap).toBe(14)
    expect(viewNode?.padding).toBe(18)

    setGap(undefined)
    setPadding(undefined)
    setDirection(undefined)
    await Promise.resolve()

    expect(viewNode?.flexDirection).toBe("column")
    expect(viewNode?.gap).toBe(0)
    expect(viewNode?.padding).toBe(0)

    dispose()
  })

  it("supports multi-window composition through createApp render", async () => {
    const app = new FakeQtApp()
    const mainWindow = createWindow({ title: "main", width: 300, height: 180 }, () => <Text>main</Text>)
    const prefsWindow = createWindow({ title: "prefs", width: 220, height: 140 }, () => <Text>prefs</Text>)

    const mounted = createApp({
      render: () => [mainWindow.render(), prefsWindow.render()],
    }).mount(app as unknown as QtApp)

    await Promise.resolve()

    const windows = allFakeNodes(app).filter((node) => node.kind === "window")
    expect(windows.map((node) => node.title)).toEqual(["main", "prefs"])
    expect(windows.every((node) => node.destroyed === false)).toBe(true)

    mounted.dispose()
    await Promise.resolve()

    expect(windows.every((node) => node.destroyed)).toBe(true)
  })

  it("disposes single-window handle without exposing raw window nodes", async () => {
    const app = new FakeQtApp()
    const mainWindow = createWindow({ title: "dispose-window", width: 320, height: 200 }, () => <Text>gone</Text>)

    const unmount = mainWindow.renderQt(app as unknown as QtApp)
    await Promise.resolve()

    const windowNode = allFakeNodes(app).find((node) => node.kind === "window" && node.title === "dispose-window")
    expect(windowNode?.destroyed).toBe(false)

    mainWindow.dispose()
    await Promise.resolve()
    await Promise.resolve()

    expect(windowNode?.destroyed).toBe(true)

    unmount()
  })

  it("creates fresh window state for each createApp factory mount", async () => {
    const app = new FakeQtApp()
    let factoryCalls = 0

    const factoryApp = createApp(() => {
      factoryCalls += 1
      const title = `factory-${factoryCalls}`
      return createWindow({ title, width: 240, height: 140 }, () => <Text>{title}</Text>)
    })

    const firstMount = factoryApp.mount(app as unknown as QtApp)
    await Promise.resolve()

    expect(
      allFakeNodes(app).some((node) => node.kind === "window" && node.title === "factory-1" && !node.destroyed),
    ).toBe(true)

    firstMount.dispose()
    await Promise.resolve()

    const secondMount = factoryApp.mount(app as unknown as QtApp)
    await Promise.resolve()

    expect(
      allFakeNodes(app).some((node) => node.kind === "window" && node.title === "factory-2" && !node.destroyed),
    ).toBe(true)

    secondMount.dispose()
  })

  it("disposes createApp mount when app.shutdown runs externally", async () => {
    const app = new FakeQtApp()
    const mainWindow = createWindow({ title: "shutdown-window", width: 300, height: 180 }, () => <Text>bye</Text>)

    createApp(mainWindow).mount(app as unknown as QtApp)
    await Promise.resolve()

    const firstWindow = allFakeNodes(app).find((node) => node.kind === "window" && node.title === "shutdown-window")
    expect(firstWindow?.destroyed).toBe(false)

    app.shutdown()
    await Promise.resolve()

    expect(firstWindow?.destroyed).toBe(true)

    const secondWindow = createWindow({ title: "restart-window", width: 240, height: 140 }, () => <Text>again</Text>)
    const mounted = createApp(secondWindow).mount(app as unknown as QtApp)
    await Promise.resolve()

    expect(
      allFakeNodes(app).some((node) => node.kind === "window" && node.title === "restart-window" && !node.destroyed),
    ).toBe(true)

    mounted.dispose()
  })
})
