import rolldownPlugin, { createQtSolidRolldownPlugin as createQtSolidRolldownPluginImpl } from "./rolldown-runtime.js"

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

export const createQtSolidRolldownPlugin = (
  input: CreateQtSolidRolldownPluginOptions = {},
): Plugin => createQtSolidRolldownPluginImpl(input) as Plugin

const qtSolidRolldownPlugin: Plugin = rolldownPlugin as Plugin

export default qtSolidRolldownPlugin
