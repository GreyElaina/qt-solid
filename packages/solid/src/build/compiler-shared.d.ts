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

export declare const DEFAULT_QT_SOLID_MODULE_NAME: string
export declare const DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME: string
export declare const DEFAULT_QT_SOLID_RUNTIME_ENTRY: string
export declare const DEFAULT_QT_SOLID_COMPILER_RT_ENTRY: string
export declare const DEFAULT_QT_SOLID_BOOTSTRAP_ENTRY: string
export declare const DEFAULT_QT_SOLID_NATIVE_MODULE_NAME: string

export declare function normalizeQtSolidFilename(id: string): string
export declare function shouldTransformQtSolidModule(filename: string): boolean
export declare function isQtSolidDependencyModule(filename: string): boolean
export declare function transformQtSolidModule(
  code: string,
  options: QtSolidCompilerOptions,
): Promise<{ code: string; map: object | null } | null>
