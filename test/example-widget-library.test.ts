import { afterEach, describe, expect, it } from "vitest"

import type { QtApp, QtNode } from "@qt-solid/core"
import { clearRegisteredQtWidgetLibraries } from "@qt-solid/core/widget-library"
import { registerCoreWidgetsLibrary } from "@qt-solid/core-widgets/widget-library"
import {
  exampleWidgetsLibrary,
  exampleWidgetsLibraryEntry,
  qtWidgetNativeBridge,
  registerExampleWidgetsLibrary,
} from "@qt-solid/example-widgets/widget-library"
import { createNativeRendererBinding } from "../packages/solid/src/runtime/native-renderer-binding"
import { FakeQtApp, type FakeQtNode } from "./mocking/fake-qt"

describe("example open-world widget library", () => {
  afterEach(() => {
    clearRegisteredQtWidgetLibraries()
  })

  it("registers stable entry and drives banner props through renderer binding", () => {
    clearRegisteredQtWidgetLibraries()
    registerCoreWidgetsLibrary({ default: true })
    registerExampleWidgetsLibrary()

    expect(exampleWidgetsLibraryEntry.library).toBe(exampleWidgetsLibrary)
    expect(exampleWidgetsLibraryEntry.nativeBridge).toBe(qtWidgetNativeBridge)
    expect(typeof qtWidgetNativeBridge.entityMap.banner?.create).toBe("function")

    const app = new FakeQtApp()
    const binding = createNativeRendererBinding(app as unknown as QtApp)

    const window = binding.createElement("window") as unknown as FakeQtNode
    const view = binding.createElement("view") as unknown as FakeQtNode
    const banner = binding.createElement("banner") as unknown as FakeQtNode

    binding.patchProp(window as unknown as QtNode, "title", undefined, "Example widgets")
    binding.patchProp(window as unknown as QtNode, "width", undefined, 320)
    binding.patchProp(window as unknown as QtNode, "height", undefined, 160)
    binding.patchProp(view as unknown as QtNode, "direction", undefined, "column")
    binding.patchProp(view as unknown as QtNode, "gap", undefined, 6)
    binding.patchProp(view as unknown as QtNode, "padding", undefined, 4)
    binding.patchProp(banner as unknown as QtNode, "text", undefined, "Open world banner")
    binding.patchProp(banner as unknown as QtNode, "minWidth", undefined, 180)
    binding.patchProp(banner as unknown as QtNode, "minHeight", undefined, 32)
    binding.patchProp(banner as unknown as QtNode, "pointSize", undefined, 18)
    binding.patchProp(banner as unknown as QtNode, "italic", undefined, true)
    binding.insertChild(app.root as unknown as QtNode, window as unknown as QtNode, undefined)
    binding.insertChild(window as unknown as QtNode, view as unknown as QtNode, undefined)
    binding.insertChild(view as unknown as QtNode, banner as unknown as QtNode, undefined)

    expect(window.kind).toBe("window")
    expect(window.title).toBe("Example widgets")
    expect(window.width).toBe(320)
    expect(window.height).toBe(160)
    expect(view.kind).toBe("view")
    expect(view.flexDirection).toBe("column")
    expect(view.gap).toBe(6)
    expect(view.padding).toBe(4)
    expect(banner.kind).toBe("label")
    expect(banner.text).toBe("Open world banner")
    expect(banner.minWidth).toBe(180)
    expect(banner.minHeight).toBe(32)
    expect(banner.fontPointSize).toBe(18)
    expect(banner.fontItalic).toBe(true)
  })
})
