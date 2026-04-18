import { describe, expect, it } from "vitest"
import { createRoot, createSignal } from "solid-js"

import { mountControllerBridge, type QtSolidBinding } from "../packages/solid/src/controller/bridge"

describe("mountControllerBridge", () => {
  it("pushes only changed signal values into binding", async () => {
    const calls: Array<[kind: string, value: string]> = []
    const events: string[] = []

    const binding: QtSolidBinding = {
      initQt(onEvent) {
        onEvent(null, "ready")
      },
      setRectangleColor(color) {
        calls.push(["color", color])
      },
      setLabelText(text) {
        calls.push(["label", text])
      },
      setLabelTextAsync() {},
    }

    let setColor!: (value: string) => string
    let setLabel!: (value: string) => string

    const dispose = createRoot((rootDispose) => {
      const [color, updateColor] = createSignal("#111827")
      const [label, updateLabel] = createSignal("hello")
      setColor = updateColor
      setLabel = updateLabel

      const unmount = mountControllerBridge(binding, { color, label }, (event) => {
        events.push(event)
      })

      return () => {
        unmount()
        rootDispose()
      }
    })

    await Promise.resolve()
    expect(events).toEqual(["ready"])
    expect(calls).toEqual([
      ["color", "#111827"],
      ["label", "hello"],
    ])

    calls.length = 0
    setColor("#3b82f6")
    await Promise.resolve()
    expect(calls).toEqual([["color", "#3b82f6"]])

    calls.length = 0
    setLabel("updated")
    await Promise.resolve()
    expect(calls).toEqual([["label", "updated"]])

    dispose()
  })
})
