import type { Plugin } from "vite"

export interface CreateQtSolidVitePluginOptions {
  moduleName?: string
  compilerRuntimeModuleName?: string
  runtimeEntry?: string
  compilerRuntimeEntry?: string
}

export declare function createQtSolidVitePlugin(
  input?: CreateQtSolidVitePluginOptions,
): Plugin

declare const qtSolidVitePlugin: Plugin
export default qtSolidVitePlugin
