import { describe, expect, it } from "vitest"
import type { ConfigEnv, Plugin, UserConfig } from "vite"

import { createQtSolidVitePlugin } from "../packages/solid/src/build/vite-runtime.js"

const TEST_HOOK_CONTEXT = {} as never
const QT_SOLID_RUNTIME_ID = "\0qt-solid:runtime"
const QT_SOLID_COMPILER_RT_ID = "\0qt-solid:compiler-rt"
const QT_SOLID_REGISTRATION_ID = "\0qt-solid:runtime-registration"

function runConfigHook(plugin: Plugin, env: ConfigEnv) {
  const hook = plugin.config
  if (!hook) {
    return null
  }

  if (typeof hook === "function") {
    return hook.call(TEST_HOOK_CONTEXT, {} as UserConfig, env)
  }

  return hook.handler.call(TEST_HOOK_CONTEXT, {} as UserConfig, env)
}

function runResolveIdHook(plugin: Plugin, source: string) {
  const hook = plugin.resolveId
  if (!hook) {
    return null
  }

  if (typeof hook === "function") {
    return hook.call(TEST_HOOK_CONTEXT, source, undefined, {
      isEntry: false,
    })
  }

  return hook.handler.call(TEST_HOOK_CONTEXT, source, undefined, {
    isEntry: false,
  })
}

function runLoadHook(plugin: Plugin, id: string) {
  const hook = plugin.load
  if (!hook) {
    return null
  }

  if (typeof hook === "function") {
    return hook.call(TEST_HOOK_CONTEXT, id)
  }

  return hook.handler.call(TEST_HOOK_CONTEXT, id)
}

describe("createQtSolidVitePlugin", () => {
  it("pins Solid runtime family to dev entries during Vite serve", async () => {
    const plugin = createQtSolidVitePlugin()
    const config = runConfigHook(plugin, { command: "serve", mode: "development" }) as {
      optimizeDeps?: { exclude?: string[] }
      resolve?: {
        alias?: Array<{ find: RegExp; replacement: string }>
        conditions?: string[]
      }
      ssr?: { noExternal?: string[] }
    } | null

    expect(config?.resolve?.conditions).toEqual(["browser", "development"])
    expect(config?.optimizeDeps?.exclude).toContain("solid-refresh")
    expect(config?.ssr?.noExternal).toContain("solid-js/universal")

    expect(await runResolveIdHook(plugin, "solid-js")).toContain("/solid-js/dist/dev.js")
    expect(await runResolveIdHook(plugin, "solid-js/store")).toContain("/solid-js/store/dist/dev.js")
    expect(await runResolveIdHook(plugin, "solid-js/universal")).toContain("/solid-js/universal/dist/dev.js")
    expect(await runResolveIdHook(plugin, "solid-js/web")).toContain("/solid-js/web/dist/dev.js")
  })

  it("uses production Solid entries outside Vite serve", async () => {
    const plugin = createQtSolidVitePlugin()
    runConfigHook(plugin, { command: "build", mode: "production" })

    expect(await runResolveIdHook(plugin, "solid-js")).toContain("/solid-js/dist/solid.js")
    expect(await runResolveIdHook(plugin, "solid-js/store")).toContain("/solid-js/store/dist/store.js")
    expect(await runResolveIdHook(plugin, "solid-js/universal")).toContain("/solid-js/universal/dist/universal.js")
    expect(await runResolveIdHook(plugin, "solid-js/web")).toContain("/solid-js/web/dist/web.js")
  })

  it("assembles a virtual runtime module that registers selected widget libraries", async () => {
    const plugin = createQtSolidVitePlugin({
      compilerRuntimeEntry: "/virtual/compiler-rt.ts",
      runtimeEntry: "/virtual/runtime.ts",
      widgetLibraries: ["@qt-solid/core-widgets", "@vendor/demo-widgets"],
    })

    expect(await runResolveIdHook(plugin, "@qt-solid/solid")).toBe(QT_SOLID_RUNTIME_ID)
    expect(await runResolveIdHook(plugin, "@qt-solid/solid/compiler-rt")).toBe(QT_SOLID_COMPILER_RT_ID)

    const runtimeModule = runLoadHook(plugin, QT_SOLID_RUNTIME_ID)
    const compilerRuntimeModule = runLoadHook(plugin, QT_SOLID_COMPILER_RT_ID)
    const registrationModule = runLoadHook(plugin, QT_SOLID_REGISTRATION_ID)
    expect(typeof runtimeModule).toBe("string")
    expect(typeof compilerRuntimeModule).toBe("string")
    expect(typeof registrationModule).toBe("string")
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
})
