import type { Plugin } from "rolldown"

export interface CreateQtSolidRolldownPluginOptions {
  bootstrap?: boolean
  bootstrapEntry?: string
  moduleName?: string
  compilerRuntimeModuleName?: string
  nativeModuleName?: string
  runtimeEntry?: string
  compilerRuntimeEntry?: string
  sourceMaps?: boolean
}

export declare function createQtSolidRolldownPlugin(
  input?: CreateQtSolidRolldownPluginOptions,
): Plugin

declare const qtSolidRolldownPlugin: Plugin
export default qtSolidRolldownPlugin
