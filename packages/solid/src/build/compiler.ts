export {
  DEFAULT_QT_SOLID_BOOTSTRAP_ENTRY,
  DEFAULT_QT_SOLID_COMPILER_RT_ENTRY,
  DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME,
  DEFAULT_QT_SOLID_MODULE_NAME,
  DEFAULT_QT_SOLID_NATIVE_MODULE_NAME,
  DEFAULT_QT_SOLID_RUNTIME_ENTRY,
  isQtSolidDependencyModule,
  normalizeQtSolidFilename,
  shouldTransformQtSolidModule,
  transformQtSolidModule,
} from "./compiler-shared.js"

export interface QtSolidCompilerOptions {
  filename: string
  hmr?: {
    bundler?: "vite"
    enabled?: boolean
  }
  moduleName?: string
  compilerRuntimeModuleName?: string
  sourceMaps?: boolean
}
