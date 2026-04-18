import { describe, expect, it } from "vitest"

import type { QtApp, QtHostEvent, QtNode } from "@qt-solid/core"
import { eventExportIds, readF64EventValue } from "@qt-solid/core-widgets/widget-library"
import type { QtSolidOwnerMetadata } from "../packages/solid/src/devtools/owner-metadata"
import { QT_SOLID_SOURCE_META_PROP } from "../packages/solid/src/devtools/source-metadata"
import { rendererInspectorStore } from "../packages/solid/src/devtools/inspector-store"
import "../packages/solid/src/runtime/register-default-widgets"
import { createNativeRendererBinding } from "../packages/solid/src/runtime/native-renderer-binding"
import { FakeQtApp, type FakeQtNode } from "./mocking/fake-qt"

describe("createNativeRendererBinding", () => {
  it("dispatches typed props and routes host events by node id", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const window = binding.createElement("window") as unknown as FakeQtNode
    const button = binding.createElement("button") as unknown as FakeQtNode
    const view = binding.createElement("view") as unknown as FakeQtNode
    const input = binding.createElement("input") as unknown as FakeQtNode
    const checkbox = binding.createElement("check") as unknown as FakeQtNode
    const slider = binding.createElement("slider") as unknown as FakeQtNode
    const doubleSpinBox = binding.createElement("doubleSpinBox") as unknown as FakeQtNode
    const events: string[] = []

    binding.insertChild(binding.root as unknown as QtNode, window as unknown as QtNode)
    binding.insertChild(window as unknown as QtNode, button as unknown as QtNode)
    binding.insertChild(window as unknown as QtNode, view as unknown as QtNode)
    binding.insertChild(window as unknown as QtNode, input as unknown as QtNode)
    binding.insertChild(window as unknown as QtNode, checkbox as unknown as QtNode)
    binding.insertChild(window as unknown as QtNode, slider as unknown as QtNode)
    binding.insertChild(window as unknown as QtNode, doubleSpinBox as unknown as QtNode)

    binding.patchProp(window as unknown as QtNode, "onCloseRequested", undefined, () => {
      events.push("close")
    })
    binding.patchProp(button as unknown as QtNode, "text", undefined, "Push")
    binding.patchProp(button as unknown as QtNode, "enabled", undefined, false)
    binding.patchProp(view as unknown as QtNode, "direction", undefined, "row")
    binding.patchProp(view as unknown as QtNode, "alignItems", undefined, "center")
    binding.patchProp(view as unknown as QtNode, "justifyContent", undefined, "center")
    binding.patchProp(view as unknown as QtNode, "gap", undefined, 8)
    binding.patchProp(view as unknown as QtNode, "padding", undefined, 12)
    binding.patchProp(input as unknown as QtNode, "placeholder", undefined, "name")
    binding.patchProp(input as unknown as QtNode, "minWidth", undefined, 140)
    binding.patchProp(input as unknown as QtNode, "minHeight", undefined, 28)
    binding.patchProp(input as unknown as QtNode, "pointSize", undefined, 12.5)
    binding.patchProp(input as unknown as QtNode, "focusPolicy", undefined, "strong-focus")
    binding.patchProp(input as unknown as QtNode, "autoFocus", undefined, true)
    binding.patchProp(input as unknown as QtNode, "cursorPosition", undefined, 5)
    binding.patchProp(input as unknown as QtNode, "selectionStart", undefined, 2)
    binding.patchProp(input as unknown as QtNode, "selectionEnd", undefined, 5)
    binding.patchProp(input as unknown as QtNode, "grow", undefined, 1)
    binding.patchProp(input as unknown as QtNode, "shrink", undefined, 0)
    binding.patchProp(slider as unknown as QtNode, "value", undefined, 25)
    binding.patchProp(slider as unknown as QtNode, "minimum", undefined, 10)
    binding.patchProp(slider as unknown as QtNode, "maximum", undefined, 90)
    binding.patchProp(slider as unknown as QtNode, "step", undefined, 5)
    binding.patchProp(slider as unknown as QtNode, "pageStep", undefined, 20)
    binding.patchProp(slider as unknown as QtNode, "focusPolicy", undefined, "tab-focus")
    binding.patchProp(slider as unknown as QtNode, "autoFocus", undefined, true)
    binding.patchProp(doubleSpinBox as unknown as QtNode, "value", undefined, 1.25)
    binding.patchProp(doubleSpinBox as unknown as QtNode, "minimum", undefined, -5)
    binding.patchProp(doubleSpinBox as unknown as QtNode, "maximum", undefined, 10.5)
    binding.patchProp(doubleSpinBox as unknown as QtNode, "step", undefined, 0.25)
    binding.patchProp(doubleSpinBox as unknown as QtNode, "focusPolicy", undefined, "click-focus")
    binding.patchProp(button as unknown as QtNode, "onClicked", undefined, () => {
      events.push("clicked")
    })
    binding.patchProp(input as unknown as QtNode, "onChanged", undefined, (value: string) => {
      events.push(`changed:${value}`)
    })
    binding.patchProp(input as unknown as QtNode, "onTextChanged", undefined, (value: string) => {
      events.push(`text:${value}`)
    })
    binding.patchProp(
      input as unknown as QtNode,
      "onCursorPositionChanged",
      undefined,
      (payload: { oldPosition: number; cursorPosition: number }) => {
        events.push(`cursor:${payload.oldPosition}->${payload.cursorPosition}`)
      },
    )
    binding.patchProp(
      input as unknown as QtNode,
      "onSelectionChanged",
      undefined,
      (payload: { selectionStart: number; selectionEnd: number }) => {
        events.push(`selection:${payload.selectionStart}-${payload.selectionEnd}`)
      },
    )
    binding.patchProp(input as unknown as QtNode, "onFocusIn", undefined, () => {
      events.push("input-focus-in")
    })
    binding.patchProp(input as unknown as QtNode, "onFocusOut", undefined, () => {
      events.push("input-focus-out")
    })
    binding.patchProp(checkbox as unknown as QtNode, "onToggled", undefined, (checked: boolean) => {
      events.push(`toggled:${checked}`)
    })
    binding.patchProp(slider as unknown as QtNode, "onValueChanged", undefined, (value: number) => {
      events.push(`slider:${value}`)
    })
    binding.patchProp(slider as unknown as QtNode, "onFocusIn", undefined, () => {
      events.push("slider-focus-in")
    })
    binding.patchProp(doubleSpinBox as unknown as QtNode, "onValueChanged", undefined, (value: number) => {
      events.push(`double:${value}`)
    })
    binding.patchProp(doubleSpinBox as unknown as QtNode, "onFocusOut", undefined, () => {
      events.push("double-focus-out")
    })

    binding.handleEvent({
      type: "listener",
      nodeId: window.id,
      listenerId: eventExportIds.onCloseRequested,
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: button.id,
      listenerId: eventExportIds.onClicked,
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onChanged,
      values: [{ path: "", kindTag: 1, stringValue: "Akashina" }],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onTextChanged,
      values: [{ path: "", kindTag: 1, stringValue: "Akashina" }],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onCursorPositionChanged,
      values: [
        { path: "oldPosition", kindTag: 3, i32Value: 2 },
        { path: "cursorPosition", kindTag: 3, i32Value: 5 },
      ],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onSelectionChanged,
      values: [
        { path: "selectionStart", kindTag: 3, i32Value: 2 },
        { path: "selectionEnd", kindTag: 3, i32Value: 5 },
      ],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onFocusIn,
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onFocusOut,
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: checkbox.id,
      listenerId: eventExportIds.onToggled,
      values: [{ path: "", kindTag: 2, boolValue: true }],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: slider.id,
      listenerId: eventExportIds.onValueChanged,
      values: [{ path: "", kindTag: 3, i32Value: 42 }],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: slider.id,
      listenerId: eventExportIds.onFocusIn,
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: doubleSpinBox.id,
      listenerId: eventExportIds.onValueChanged,
      values: [{ path: "", kindTag: 4, f64Value: 2.5 }],
    } as QtHostEvent)
    binding.handleEvent({
      type: "listener",
      nodeId: doubleSpinBox.id,
      listenerId: eventExportIds.onFocusOut,
    } as QtHostEvent)

    expect(button.text).toBe("Push")
    expect(button.enabled).toBe(false)
    expect(view.flexDirection).toBe("row")
    expect(view.alignItems).toBe("center")
    expect(view.justifyContent).toBe("center")
    expect(view.gap).toBe(8)
    expect(view.padding).toBe(12)
    expect(input.placeholder).toBe("name")
    expect(input.minWidth).toBe(140)
    expect(input.minHeight).toBe(28)
    expect(input.fontPointSize).toBe(12.5)
    expect(input.focusPolicy).toBe("strong-focus")
    expect(input.autoFocus).toBe(true)
    expect(input.cursorPosition).toBe(5)
    expect(input.selectionStart).toBe(2)
    expect(input.selectionEnd).toBe(5)
    expect(input.flexGrow).toBe(1)
    expect(input.flexShrink).toBe(0)
    expect(slider.rangeValue).toBe(25)
    expect(slider.rangeMinimum).toBe(10)
    expect(slider.rangeMaximum).toBe(90)
    expect(slider.rangeStep).toBe(5)
    expect(slider.rangePageStep).toBe(20)
    expect(slider.focusPolicy).toBe("tab-focus")
    expect(slider.autoFocus).toBe(true)
    expect(doubleSpinBox.rangeValue).toBe(1.25)
    expect(doubleSpinBox.rangeMinimum).toBe(-5)
    expect(doubleSpinBox.rangeMaximum).toBe(10.5)
    expect(doubleSpinBox.rangeStep).toBe(0.25)
    expect(doubleSpinBox.focusPolicy).toBe("click-focus")
    expect(events).toEqual([
      "close",
      "clicked",
      "changed:Akashina",
      "text:Akashina",
      "cursor:2->5",
      "selection:2-5",
      "input-focus-in",
      "input-focus-out",
      "toggled:true",
      "slider:42",
      "slider-focus-in",
      "double:2.5",
      "double-focus-out",
    ])
  })

  it("skips controlled text write-back when listener payload already matches next prop", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const input = binding.createElement("input") as unknown as FakeQtNode
    let value = "Akashina"

    binding.patchProp(input as unknown as QtNode, "text", undefined, value)
    binding.patchProp(input as unknown as QtNode, "onChanged", undefined, (next: string) => {
      value = next
    })

    const beforeApplyCount = input.appliedProps.length

    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onChanged,
      values: [{ path: "", kindTag: 1, stringValue: "Akashina!" }],
    } as QtHostEvent)

    binding.patchProp(input as unknown as QtNode, "text", value, value)

    expect(input.appliedProps.slice(beforeApplyCount)).toEqual([])
  })

  it("skips controlled checked write-back when listener payload already matches next prop", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const check = binding.createElement("check") as unknown as FakeQtNode
    let checked = true

    binding.patchProp(check as unknown as QtNode, "checked", undefined, checked)
    binding.patchProp(check as unknown as QtNode, "onToggled", undefined, (next: boolean) => {
      checked = next
    })

    const beforeApplyCount = check.appliedProps.length

    binding.handleEvent({
      type: "listener",
      nodeId: check.id,
      listenerId: eventExportIds.onToggled,
      values: [{ path: "", kindTag: 2, boolValue: true }],
    } as QtHostEvent)

    binding.patchProp(check as unknown as QtNode, "checked", checked, checked)

    expect(check.appliedProps.slice(beforeApplyCount)).toEqual([])
  })

  it("skips controlled value write-back when listener payload already matches next prop", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const slider = binding.createElement("slider") as unknown as FakeQtNode
    let value = 25

    binding.patchProp(slider as unknown as QtNode, "value", undefined, value)
    binding.patchProp(slider as unknown as QtNode, "onValueChanged", undefined, (next: number) => {
      value = next
    })

    const beforeApplyCount = slider.appliedProps.length

    binding.handleEvent({
      type: "listener",
      nodeId: slider.id,
      listenerId: eventExportIds.onValueChanged,
      values: [{ path: "", kindTag: 3, i32Value: 42 }],
    } as QtHostEvent)

    binding.patchProp(slider as unknown as QtNode, "value", 25, value)

    expect(slider.appliedProps.slice(beforeApplyCount)).toEqual([])
  })

  it("skips controlled selection write-back when object payload already matches next props", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const input = binding.createElement("input") as unknown as FakeQtNode
    let selection = { cursorPosition: 0, selectionStart: 2, selectionEnd: 5 }

    binding.patchProp(input as unknown as QtNode, "cursorPosition", undefined, selection.cursorPosition)
    binding.patchProp(input as unknown as QtNode, "selectionStart", undefined, selection.selectionStart)
    binding.patchProp(input as unknown as QtNode, "selectionEnd", undefined, selection.selectionEnd)
    binding.patchProp(
      input as unknown as QtNode,
      "onSelectionChanged",
      undefined,
      (payload: { selectionStart: number; selectionEnd: number }) => {
        selection = {
          ...selection,
          selectionStart: payload.selectionStart,
          selectionEnd: payload.selectionEnd,
        }
      },
    )

    const beforeApplyCount = input.appliedProps.length

    binding.handleEvent({
      type: "listener",
      nodeId: input.id,
      listenerId: eventExportIds.onSelectionChanged,
      values: [
        { path: "selectionStart", kindTag: 3, i32Value: 3 },
        { path: "selectionEnd", kindTag: 3, i32Value: 6 },
      ],
    } as QtHostEvent)

    binding.patchProp(input as unknown as QtNode, "selectionStart", 2, selection.selectionStart)
    binding.patchProp(input as unknown as QtNode, "selectionEnd", 5, selection.selectionEnd)

    expect(input.appliedProps.slice(beforeApplyCount)).toEqual([])
  })

  it("resets flat layout props to defaults when reactive props disappear", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const view = binding.createElement("view") as unknown as FakeQtNode

    binding.insertChild(binding.root as unknown as QtNode, view as unknown as QtNode)

    binding.patchProp(view as unknown as QtNode, "direction", undefined, "row")
    binding.patchProp(view as unknown as QtNode, "gap", undefined, 8)
    binding.patchProp(view as unknown as QtNode, "padding", undefined, 12)

    expect(view.flexDirection).toBe("row")
    expect(view.gap).toBe(8)
    expect(view.padding).toBe(12)

    binding.patchProp(view as unknown as QtNode, "direction", "row", undefined)
    binding.patchProp(view as unknown as QtNode, "gap", 8, undefined)
    binding.patchProp(view as unknown as QtNode, "padding", 12, undefined)

    expect(view.flexDirection).toBe("column")
    expect(view.gap).toBe(0)
    expect(view.padding).toBe(0)
  })

  it("reads f64 listener payload values", () => {
    const values: Extract<QtHostEvent, { type: "listener" }>["values"] = [
      { path: "ratio", kindTag: 4, f64Value: 1.25 },
    ]

    expect(
      readF64EventValue(values, "ratio"),
    ).toBe(1.25)
  })

  it("records source metadata on renderer nodes without leaking to native props", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const text = binding.createElement("text") as unknown as FakeQtNode
    binding.insertChild(binding.root as unknown as QtNode, text as unknown as QtNode)

    binding.patchProp(text as unknown as QtNode, QT_SOLID_SOURCE_META_PROP, undefined, {
      fileName: "examples/counter/app.tsx",
      lineNumber: 18,
      columnNumber: 9,
    })

    expect(text.appliedProps).toEqual([])
    expect(rendererInspectorStore.snapshot().nodes.find((node) => node.id === text.id)?.source).toEqual({
      fileName: "examples/counter/app.tsx",
      lineNumber: 18,
      columnNumber: 9,
    })
  })

  it("records owner metadata on renderer nodes without leaking to native props", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const text = binding.createElement("text") as unknown as FakeQtNode
    binding.insertChild(binding.root as unknown as QtNode, text as unknown as QtNode)

    binding.attachDebugMetadata?.(text as unknown as QtNode, {
      owner: {
        ownerStack: [
          {
            componentName: "CounterPanel",
            source: {
              fileName: "examples/counter/app.tsx",
              lineNumber: 10,
              columnNumber: 3,
            },
          },
          {
            componentName: "Text",
            source: {
              fileName: "examples/counter/app.tsx",
              lineNumber: 11,
              columnNumber: 5,
            },
          },
        ],
      } satisfies QtSolidOwnerMetadata,
    })

    expect(text.appliedProps).toEqual([])
    expect(rendererInspectorStore.snapshot().nodes.find((node) => node.id === text.id)?.owner).toEqual({
      ownerStack: [
        {
          componentName: "CounterPanel",
          source: {
            fileName: "examples/counter/app.tsx",
            lineNumber: 10,
            columnNumber: 3,
          },
        },
        {
          componentName: "Text",
          source: {
            fileName: "examples/counter/app.tsx",
            lineNumber: 11,
            columnNumber: 5,
          },
        },
      ],
    })
  })

  it("canonicalizes tree query wrappers by node id", () => {
    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const parent = binding.createElement("view") as unknown as FakeQtNode
    const child = binding.createElement("label") as unknown as FakeQtNode

    binding.insertChild(parent as unknown as QtNode, child as unknown as QtNode)

    const first = binding.getFirstChild(parent as unknown as QtNode)
    const again = binding.getFirstChild(parent as unknown as QtNode)
    const parentOfChild = binding.getParent(child as unknown as QtNode)

    expect(first).toBe(again)
    expect(parentOfChild).toBe(parent as unknown as QtNode)
  })
})
