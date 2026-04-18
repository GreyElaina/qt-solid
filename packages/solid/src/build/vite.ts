import vitePlugin, { createQtSolidVitePlugin as createQtSolidVitePluginImpl } from "./vite-runtime.js"

import type { Plugin } from "vite"

export interface CreateQtSolidVitePluginOptions {
  moduleName?: string
  compilerRuntimeModuleName?: string
  runtimeEntry?: string
  compilerRuntimeEntry?: string
  widgetLibraries?: readonly string[]
}

export const createQtSolidVitePlugin = (
  input: CreateQtSolidVitePluginOptions = {},
): Plugin => createQtSolidVitePluginImpl(input) as Plugin

const qtSolidVitePlugin: Plugin = vitePlugin as Plugin

export default qtSolidVitePlugin
