import { createRequire } from "node:module"
import { resolve as resolvePath } from "node:path"

import {
  DEFAULT_QT_SOLID_BOOTSTRAP_ENTRY,
  DEFAULT_QT_SOLID_COMPILER_RT_ENTRY,
  DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME,
  DEFAULT_QT_SOLID_MODULE_NAME,
  DEFAULT_QT_SOLID_NATIVE_MODULE_NAME,
  DEFAULT_QT_SOLID_RUNTIME_ENTRY,
  normalizeQtSolidFilename,
  transformQtSolidModule,
} from "./compiler-shared.js"

export const QT_SOLID_RUNTIME_ID = "\0qt-solid:runtime"
export const QT_SOLID_COMPILER_RT_ID = "\0qt-solid:compiler-rt"

const require = createRequire(import.meta.url)
const SOLID_JS_RUNTIME_ENTRY = require.resolve("solid-js/dist/solid.js")
const SOLID_JS_STORE_RUNTIME_ENTRY = require.resolve("solid-js/store/dist/store.js")
const QT_SOLID_BOOTSTRAP_ID = "\0qt-solid:bootstrap"

function resolveBootstrapInput(input) {
  if (typeof input !== "string") {
    throw new Error("qt-solid rolldown bootstrap mode requires single string input")
  }

  return resolvePath(input)
}

export function createQtSolidRolldownPlugin(input = {}) {
  const bootstrapEnabled = input.bootstrap ?? true
  const bootstrapEntry = input.bootstrapEntry ?? DEFAULT_QT_SOLID_BOOTSTRAP_ENTRY
  const moduleName = input.moduleName ?? DEFAULT_QT_SOLID_MODULE_NAME
  const compilerRuntimeModuleName =
    input.compilerRuntimeModuleName ?? DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME
  const nativeModuleName = input.nativeModuleName ?? DEFAULT_QT_SOLID_NATIVE_MODULE_NAME
  const nativePrimitiveModuleName = `${nativeModuleName}/native`
  const nativeGeneratedModuleName = `${nativeModuleName}/native/generated`
  const runtimeEntry = input.runtimeEntry ?? DEFAULT_QT_SOLID_RUNTIME_ENTRY
  const compilerRuntimeEntry = input.compilerRuntimeEntry ?? DEFAULT_QT_SOLID_COMPILER_RT_ENTRY
  const sourceMaps = input.sourceMaps ?? true
  let entryPath

  return {
    name: "qt-solid-rolldown-plugin",
    options(options) {
      if (!bootstrapEnabled) {
        return null
      }

      entryPath = resolveBootstrapInput(options.input)
      return {
        ...options,
        input: QT_SOLID_BOOTSTRAP_ID,
      }
    },
    resolveId(id, importer) {
      if (id === QT_SOLID_BOOTSTRAP_ID) {
        return QT_SOLID_BOOTSTRAP_ID
      }

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

      if (id === nativeModuleName) {
        return {
          id: nativeModuleName,
          external: true,
        }
      }

      if (id === nativePrimitiveModuleName) {
        return {
          id: nativePrimitiveModuleName,
          external: true,
        }
      }

      if (id === nativeGeneratedModuleName) {
        return {
          id: nativeGeneratedModuleName,
          external: true,
        }
      }

      if (id === "solid-js") {
        return SOLID_JS_RUNTIME_ENTRY
      }

      if (id === "solid-js/store") {
        return SOLID_JS_STORE_RUNTIME_ENTRY
      }

      if (/native\/index\.js$/.test(id)) {
        return {
          id: nativePrimitiveModuleName,
          external: true,
        }
      }

      if (/native\/generated\.js$/.test(id)) {
        return {
          id: nativeGeneratedModuleName,
          external: true,
        }
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

      if (id !== QT_SOLID_BOOTSTRAP_ID) {
        return null
      }

      if (!entryPath) {
        throw new Error("qt-solid rolldown bootstrap entry path missing")
      }

      return [
        `import entry from ${JSON.stringify(entryPath)}`,
        `import { startQtSolidApp } from ${JSON.stringify(bootstrapEntry)}`,
        "",
        "startQtSolidApp(entry)",
      ].join("\n")
    },
    async transform(code, id) {
      const filename = normalizeQtSolidFilename(id)
      const transformed = await transformQtSolidModule(code, {
        filename,
        compilerRuntimeModuleName,
        moduleName,
        sourceMaps,
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

const qtSolidRolldownPlugin = createQtSolidRolldownPlugin()

export default qtSolidRolldownPlugin
