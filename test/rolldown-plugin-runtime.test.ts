import { describe, expect, it } from "vitest"
import type { Plugin } from "rolldown"

import { createQtSolidRolldownPlugin } from "../packages/solid/src/build/rolldown-runtime.js"

const TEST_HOOK_CONTEXT = {} as never
const QT_SOLID_RUNTIME_ID = "\0qt-solid:runtime"
const QT_SOLID_COMPILER_RT_ID = "\0qt-solid:compiler-rt"

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
  it("re-exports the qt-solid runtime entries through virtual modules", () => {
    const plugin = createQtSolidRolldownPlugin({
      bootstrap: false,
      compilerRuntimeEntry: "/virtual/compiler-rt.ts",
      runtimeEntry: "/virtual/runtime.ts",
    })

    expect(runResolveIdHook(plugin, "@qt-solid/solid")).toBe(QT_SOLID_RUNTIME_ID)
    expect(runResolveIdHook(plugin, "@qt-solid/solid/compiler-rt")).toBe(QT_SOLID_COMPILER_RT_ID)

    const runtimeModule = runLoadHook(plugin, QT_SOLID_RUNTIME_ID)
    const compilerRuntimeModule = runLoadHook(plugin, QT_SOLID_COMPILER_RT_ID)
    expect(runtimeModule).toBe('export * from "/virtual/runtime.ts"\n')
    expect(compilerRuntimeModule).toBe('export * from "/virtual/compiler-rt.ts"\n')
  })

  it("marks the qt-solid native module as external", () => {
    const plugin = createQtSolidRolldownPlugin({ bootstrap: false })

    expect(runResolveIdHook(plugin, "@qt-solid/core")).toEqual({
      id: "@qt-solid/core",
      external: true,
    })
    expect(runResolveIdHook(plugin, "@qt-solid/core/native")).toEqual({
      id: "@qt-solid/core/native",
      external: true,
    })
  })
})
