import { describe, expect, it } from "vitest"
import type { Plugin } from "rolldown"

import { createQtSolidRolldownPlugin } from "../packages/solid/src/build/rolldown-runtime.js"

const TEST_HOOK_CONTEXT = {} as never
const QT_SOLID_RUNTIME_ID = "\0qt-solid:runtime"
const QT_SOLID_COMPILER_RT_ID = "\0qt-solid:compiler-rt"
const QT_SOLID_REGISTRATION_ID = "\0qt-solid:runtime-registration"

function runResolveIdHook(plugin: Plugin, source: string) {
  const hook = plugin.resolveId
  if (typeof hook !== "function") {
    throw new Error("rolldown plugin resolveId hook is missing")
  }

  return (hook as (...args: unknown[]) => unknown).call(TEST_HOOK_CONTEXT, source, undefined)
}

function runLoadHook(plugin: Plugin, id: string) {
  const hook = plugin.load
  if (typeof hook !== "function") {
    throw new Error("rolldown plugin load hook is missing")
  }

  return (hook as (...args: unknown[]) => unknown).call(TEST_HOOK_CONTEXT, id)
}

describe("createQtSolidRolldownPlugin", () => {
  it("assembles a virtual runtime module that registers selected widget libraries", () => {
    const plugin = createQtSolidRolldownPlugin({
      bootstrap: false,
      compilerRuntimeEntry: "/virtual/compiler-rt.ts",
      runtimeEntry: "/virtual/runtime.ts",
      widgetLibraries: ["@qt-solid/core-widgets", "@vendor/demo-widgets"],
    })

    expect(runResolveIdHook(plugin, "@qt-solid/solid")).toBe(QT_SOLID_RUNTIME_ID)
    expect(runResolveIdHook(plugin, "@qt-solid/solid/compiler-rt")).toBe(QT_SOLID_COMPILER_RT_ID)

    const runtimeModule = runLoadHook(plugin, QT_SOLID_RUNTIME_ID)
    const compilerRuntimeModule = runLoadHook(plugin, QT_SOLID_COMPILER_RT_ID)
    const registrationModule = runLoadHook(plugin, QT_SOLID_REGISTRATION_ID)
    expect(typeof runtimeModule).toBe("string")
    expect(typeof compilerRuntimeModule).toBe("string")
    expect(typeof registrationModule).toBe("string")
    expect(registrationModule).toContain(
      'import { registerQtWidgetLibraryEntry } from "@qt-solid/core/widget-library"',
    )
    expect(registrationModule).toContain(
      'import { qtWidgetLibraryEntry as qtWidgetLibraryEntry0 } from "@qt-solid/core-widgets/widget-library"',
    )
    expect(registrationModule).toContain(
      'import { qtWidgetLibraryEntry as qtWidgetLibraryEntry1 } from "@vendor/demo-widgets/widget-library"',
    )
    expect(registrationModule).toContain(
      "registerQtWidgetLibraryEntry(qtWidgetLibraryEntry0, { default: true })",
    )
    expect(registrationModule).toContain(
      "registerQtWidgetLibraryEntry(qtWidgetLibraryEntry1, { default: false })",
    )
    expect(runtimeModule).toContain(`import ${JSON.stringify(QT_SOLID_REGISTRATION_ID)}`)
    expect(runtimeModule).toContain('export * from "/virtual/runtime.ts"')
    expect(compilerRuntimeModule).toContain(`import ${JSON.stringify(QT_SOLID_REGISTRATION_ID)}`)
    expect(compilerRuntimeModule).toContain('export * from "/virtual/compiler-rt.ts"')
  })

  it("defaults to core-widgets when no explicit library list is provided", () => {
    const plugin = createQtSolidRolldownPlugin({
      bootstrap: false,
      runtimeEntry: "/virtual/runtime.ts",
    })

    const registrationModule = runLoadHook(plugin, QT_SOLID_REGISTRATION_ID)
    expect(registrationModule).toContain(
      'import { qtWidgetLibraryEntry as qtWidgetLibraryEntry0 } from "@qt-solid/core-widgets/widget-library"',
    )
  })
})
