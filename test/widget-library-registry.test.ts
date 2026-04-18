import { describe, expect, it } from "vitest"

import {
  clearRegisteredQtWidgetLibraries,
  defineQtWidgetLibrary,
  defineQtWidgetLibraryEntry,
  libraryNativeBridgeOf,
  registeredQtWidgetLibraryEntries,
  registerQtWidgetLibrary,
  registerQtWidgetLibraryEntry,
  registeredQtWidgetLibraries,
  resolveQtWidgetLibraryEntryForIntrinsic,
  resolveDefaultQtWidgetLibraryEntry,
  resolveDefaultQtWidgetLibrary,
  resolveDefaultQtWidgetLibraryNativeBridge,
  resolveQtWidgetLibraryNativeBridge,
  type QtWidgetKind,
} from "@qt-solid/core/widget-library"
import {
  coreWidgetsLibrary,
  coreWidgetsLibraryEntry,
  registerCoreWidgetsLibrary,
  qtWidgetNativeBridge,
} from "@qt-solid/core-widgets/widget-library"

describe("widget library registry", () => {
  const viewKind = "view" as QtWidgetKind
  const textKind = "text" as QtWidgetKind

  it("exposes builtin library as stable package entry", () => {
    expect(coreWidgetsLibraryEntry.library).toBe(coreWidgetsLibrary)
    expect(coreWidgetsLibraryEntry.nativeBridge).toBe(qtWidgetNativeBridge)
    expect(typeof qtWidgetNativeBridge.entityMap.window?.create).toBe("function")
  })

  it("registers builtin library as explicit assembly input", () => {
    clearRegisteredQtWidgetLibraries()
    registerCoreWidgetsLibrary({ default: true })

    expect(resolveDefaultQtWidgetLibrary()).toBe(coreWidgetsLibrary)
    expect(resolveDefaultQtWidgetLibraryEntry()).toBe(coreWidgetsLibraryEntry)
    expect(resolveDefaultQtWidgetLibraryNativeBridge()).toBe(qtWidgetNativeBridge)
    expect(registeredQtWidgetLibraries()).toContain(coreWidgetsLibrary)
    expect(registeredQtWidgetLibraryEntries()).toContain(coreWidgetsLibraryEntry)
  })

  it("derives resolved metadata from stable widget library contract", () => {
    const testLibrary = defineQtWidgetLibrary({
      name: "test-library",
      intrinsicKind: { alpha: viewKind, beta: textKind },
      eventExportIds: { onClicked: 2, onChanged: 1 },
      metadata: {
        kinds: [],
        props: [
          { jsName: "text", propId: 1 },
          { jsName: "title", propId: 2 },
        ],
        eventExports: [],
      },
      collectControlledPropValues: () => [],
      dispatchNativeEvent: () => {},
      isEventExportProp: () => false,
      createIntrinsicNode: () => undefined,
    })

    expect(testLibrary.metadata.kinds).toEqual([
      { intrinsicName: "alpha", nativeKind: viewKind },
      { intrinsicName: "beta", nativeKind: textKind },
    ])
    expect(testLibrary.metadata.props).toEqual([
      { jsName: "text", propId: 1 },
      { jsName: "title", propId: 2 },
    ])
    expect(testLibrary.metadata.eventExports).toEqual([
      { exportName: "onChanged", exportId: 1 },
      { exportName: "onClicked", exportId: 2 },
    ])
  })

  it("registerQtWidgetLibrary synthesizes an entry from the provided native bridge", () => {
    clearRegisteredQtWidgetLibraries()

    const testLibrary = defineQtWidgetLibrary({
      name: "compat-library",
      intrinsicKind: { alpha: viewKind },
      eventExportIds: {},
      collectControlledPropValues: () => [],
      dispatchNativeEvent: () => {},
      isEventExportProp: () => false,
      createIntrinsicNode: () => undefined,
    })
    const compatBridge = { entityMap: {} }

    const compatEntry = defineQtWidgetLibraryEntry({ library: testLibrary, nativeBridge: compatBridge })

    registerQtWidgetLibrary(testLibrary, { default: true, nativeBridge: compatBridge })

    expect(resolveDefaultQtWidgetLibrary()).toBe(testLibrary)
    expect(resolveDefaultQtWidgetLibraryEntry()).toEqual(compatEntry)
  })

  it("stores native bridges on registered entries", () => {
    clearRegisteredQtWidgetLibraries()

    const testLibrary = defineQtWidgetLibrary({
      name: "module-library",
      intrinsicKind: { alpha: viewKind },
      eventExportIds: {},
      collectControlledPropValues: () => [],
      dispatchNativeEvent: () => {},
      isEventExportProp: () => false,
      createIntrinsicNode: () => undefined,
    })

    const moduleBridge = { entityMap: {} }
    const moduleEntry = defineQtWidgetLibraryEntry({ library: testLibrary, nativeBridge: moduleBridge })

    registerQtWidgetLibraryEntry(moduleEntry, { default: true })

    expect(resolveDefaultQtWidgetLibraryNativeBridge()).toBe(moduleBridge)
    expect(resolveQtWidgetLibraryNativeBridge(testLibrary.name)).toBe(moduleBridge)
    expect(libraryNativeBridgeOf(testLibrary)).toBe(moduleBridge)
  })

  it("resolves intrinsic owner across registered libraries and rejects duplicates", () => {
    clearRegisteredQtWidgetLibraries()

    const alphaLibrary = defineQtWidgetLibrary({
      name: "alpha-library",
      intrinsicKind: { alpha: viewKind },
      eventExportIds: {},
      collectControlledPropValues: () => [],
      dispatchNativeEvent: () => {},
      isEventExportProp: () => false,
      createIntrinsicNode: () => undefined,
    })
    const betaLibrary = defineQtWidgetLibrary({
      name: "beta-library",
      intrinsicKind: { beta: textKind },
      eventExportIds: {},
      collectControlledPropValues: () => [],
      dispatchNativeEvent: () => {},
      isEventExportProp: () => false,
      createIntrinsicNode: () => undefined,
    })
    const duplicateLibrary = defineQtWidgetLibrary({
      name: "duplicate-library",
      intrinsicKind: { beta: viewKind },
      eventExportIds: {},
      collectControlledPropValues: () => [],
      dispatchNativeEvent: () => {},
      isEventExportProp: () => false,
      createIntrinsicNode: () => undefined,
    })

    registerQtWidgetLibraryEntry(
      defineQtWidgetLibraryEntry({ library: alphaLibrary, nativeBridge: { entityMap: {} } }),
      { default: true },
    )
    registerQtWidgetLibraryEntry(
      defineQtWidgetLibraryEntry({ library: betaLibrary, nativeBridge: { entityMap: {} } }),
    )

    expect(resolveQtWidgetLibraryEntryForIntrinsic("alpha").library).toBe(alphaLibrary)
    expect(resolveQtWidgetLibraryEntryForIntrinsic("beta").library).toBe(betaLibrary)
    expect(() => resolveQtWidgetLibraryEntryForIntrinsic("missing")).toThrow(
      "Qt intrinsic is not registered: missing",
    )
    expect(() =>
      registerQtWidgetLibraryEntry(
        defineQtWidgetLibraryEntry({ library: duplicateLibrary, nativeBridge: { entityMap: {} } }),
      ),
    ).toThrow("Qt intrinsic beta is already registered by beta-library")
  })
})
