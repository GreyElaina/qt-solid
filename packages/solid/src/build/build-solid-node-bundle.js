import { build } from "rolldown"

import { createQtSolidRolldownPlugin } from "./rolldown-runtime.js"

export async function buildSolidNodeBundle(options) {
  await build({
    input: options.entryPath,
    output: {
      file: options.outfile,
      format: "esm",
      sourcemap: true,
    },
    platform: "node",
    plugins: [
      createQtSolidRolldownPlugin({
        bootstrap: options.bootstrap ?? false,
        compilerRuntimeEntry: options.compilerRuntimeEntry,
        compilerRuntimeModuleName: options.compilerRuntimeModuleName,
        moduleName: options.moduleName,
        nativeModuleName: options.nativeModuleName,
        runtimeEntry: options.runtimeEntry,
        widgetLibraries: options.widgetLibraries,
      }),
    ],
    resolve: {
      conditionNames: ["browser"],
    },
    write: true,
  })
}
