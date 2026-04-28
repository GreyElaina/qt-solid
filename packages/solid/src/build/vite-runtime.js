import { createRequire } from "node:module"

import {
  DEFAULT_QT_SOLID_COMPILER_RT_ENTRY,
  DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME,
  DEFAULT_QT_SOLID_MODULE_NAME,
  DEFAULT_QT_SOLID_RUNTIME_ENTRY,
  normalizeQtSolidFilename,
  transformQtSolidModule,
} from "./compiler-shared.js"

export const QT_SOLID_RUNTIME_ID = "\0qt-solid:runtime"
export const QT_SOLID_COMPILER_RT_ID = "\0qt-solid:compiler-rt"

const require = createRequire(import.meta.url)
const SOLID_JS_BROWSER_ENTRY = require.resolve("solid-js/dist/solid.js")
const SOLID_JS_DEV_ENTRY = require.resolve("solid-js/dist/dev.js")
const SOLID_JS_STORE_BROWSER_ENTRY = require.resolve("solid-js/store/dist/store.js")
const SOLID_JS_STORE_DEV_ENTRY = require.resolve("solid-js/store/dist/dev.js")
const SOLID_JS_UNIVERSAL_BROWSER_ENTRY = require.resolve("solid-js/universal/dist/universal.js")
const SOLID_JS_UNIVERSAL_DEV_ENTRY = require.resolve("solid-js/universal/dist/dev.js")
const SOLID_JS_WEB_BROWSER_ENTRY = require.resolve("solid-js/web/dist/web.js")
const SOLID_JS_WEB_DEV_ENTRY = require.resolve("solid-js/web/dist/dev.js")

function createSolidRuntimeEntries(devServer) {
  return {
    "solid-js": devServer ? SOLID_JS_DEV_ENTRY : SOLID_JS_BROWSER_ENTRY,
    "solid-js/store": devServer ? SOLID_JS_STORE_DEV_ENTRY : SOLID_JS_STORE_BROWSER_ENTRY,
    "solid-js/universal": devServer ? SOLID_JS_UNIVERSAL_DEV_ENTRY : SOLID_JS_UNIVERSAL_BROWSER_ENTRY,
    "solid-js/web": devServer ? SOLID_JS_WEB_DEV_ENTRY : SOLID_JS_WEB_BROWSER_ENTRY,
  }
}

function createSolidRuntimeAliases(devServer) {
  const entries = createSolidRuntimeEntries(devServer)
  return [
    { find: /^solid-js\/universal$/, replacement: entries["solid-js/universal"] },
    { find: /^solid-js\/store$/, replacement: entries["solid-js/store"] },
    { find: /^solid-js\/web$/, replacement: entries["solid-js/web"] },
    { find: /^solid-js$/, replacement: entries["solid-js"] },
  ]
}

export function createQtSolidVitePlugin(input = {}) {
  const moduleName = input.moduleName ?? DEFAULT_QT_SOLID_MODULE_NAME
  const compilerRuntimeModuleName =
    input.compilerRuntimeModuleName ?? DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME
  const runtimeEntry = input.runtimeEntry ?? DEFAULT_QT_SOLID_RUNTIME_ENTRY
  const compilerRuntimeEntry = input.compilerRuntimeEntry ?? DEFAULT_QT_SOLID_COMPILER_RT_ENTRY
  let devServer = false

  return {
    name: "qt-solid-vite-plugin",
    enforce: "pre",
    config(_, env) {
      devServer = env.command === "serve"
      const alias = createSolidRuntimeAliases(devServer)
      if (!devServer) {
        return {
          resolve: {
            alias,
          },
        }
      }

      return {
        optimizeDeps: {
          exclude: ["solid-refresh", "solid-js", "solid-js/store", "solid-js/universal", "solid-js/web"],
        },
        resolve: {
          alias,
          conditions: ["browser", "development"],
        },
        ssr: {
          noExternal: ["solid-refresh", "solid-js", "solid-js/store", "solid-js/universal", "solid-js/web"],
        },
      }
    },
    resolveId(id) {
      const entries = createSolidRuntimeEntries(devServer)

      if (id === QT_SOLID_RUNTIME_ID) {
        return QT_SOLID_RUNTIME_ID
      }

      if (id === QT_SOLID_COMPILER_RT_ID) {
        return QT_SOLID_COMPILER_RT_ID
      }

      if (id === moduleName) {
        return QT_SOLID_RUNTIME_ID
      }

      if (id === compilerRuntimeModuleName) {
        return QT_SOLID_COMPILER_RT_ID
      }

      if (id in entries) {
        return entries[id]
      }

      return null
    },
    load(id) {
      if (id === QT_SOLID_RUNTIME_ID) {
        return `export * from ${JSON.stringify(runtimeEntry)}\n`
      }

      if (id === QT_SOLID_COMPILER_RT_ID) {
        return `export * from ${JSON.stringify(compilerRuntimeEntry)}\n`
      }

      return null
    },
    async transform(code, id) {
      const transformed = await transformQtSolidModule(code, {
        filename: normalizeQtSolidFilename(id),
        compilerRuntimeModuleName,
        hmr: devServer
          ? {
              bundler: "vite",
              enabled: true,
            }
          : undefined,
        moduleName,
        sourceMaps: true,
      })

      if (!transformed) {
        return null
      }

      return {
        code: transformed.code,
        map: transformed.map,
      }
    },
  }
}

const qtSolidVitePlugin = createQtSolidVitePlugin()

export default qtSolidVitePlugin
