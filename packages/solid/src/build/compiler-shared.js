import { transformAsync } from "@babel/core"
import ts from "@babel/preset-typescript"
import solid from "babel-preset-solid"
import { isAbsolute, relative as relativePath, resolve, sep as pathSeparator } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import solidRefresh from "solid-refresh/babel"

import { QT_SOLID_SOURCE_META_PROP } from "../devtools/source-metadata.ts"

export const DEFAULT_QT_SOLID_MODULE_NAME = "@qt-solid/solid"
export const DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME = "@qt-solid/solid/compiler-rt"
export const DEFAULT_QT_SOLID_RUNTIME_ENTRY = fileURLToPath(new URL("../index.ts", import.meta.url))
export const DEFAULT_QT_SOLID_COMPILER_RT_ENTRY = fileURLToPath(new URL("../compiler-rt.ts", import.meta.url))
export const DEFAULT_QT_SOLID_BOOTSTRAP_ENTRY = fileURLToPath(new URL("../entry/bootstrap.ts", import.meta.url))
export const DEFAULT_QT_SOLID_NATIVE_MODULE_NAME = "@qt-solid/core"

export function normalizeQtSolidFilename(id) {
  return id.split("?", 1)[0] ?? id
}

function normalizeQtSolidSourceFilename(filename) {
  const relativeFilename = relativePath(process.cwd(), filename)
  const displayFilename = !relativeFilename || relativeFilename.startsWith("..") ? filename : relativeFilename
  return displayFilename.split(pathSeparator).join("/")
}

function canonicalQtSolidFilename(filename) {
  return isAbsolute(filename) ? filename : resolve(process.cwd(), filename)
}

function qtSolidDirectoryUrl(pathname) {
  const href = pathToFileURL(pathname).href
  return href.endsWith("/") ? href : `${href}/`
}

function createQtSolidSourceMetadataExpression(t, filename, lineNumber, columnNumber) {
  const canonicalFilename = canonicalQtSolidFilename(filename)
  return t.objectExpression([
    t.objectProperty(t.identifier("fileName"), t.stringLiteral(normalizeQtSolidSourceFilename(canonicalFilename))),
    t.objectProperty(t.identifier("lineNumber"), t.numericLiteral(lineNumber)),
    t.objectProperty(t.identifier("columnNumber"), t.numericLiteral(columnNumber)),
    t.objectProperty(t.identifier("fileUrl"), t.stringLiteral(pathToFileURL(canonicalFilename).href)),
    t.objectProperty(t.identifier("projectRootUrl"), t.stringLiteral(qtSolidDirectoryUrl(process.cwd()))),
  ])
}

function hasQtSolidSourceMetadataAttribute(t, attributes) {
  return attributes.some(
    (attribute) => t.isJSXAttribute(attribute) && t.isJSXIdentifier(attribute.name, { name: QT_SOLID_SOURCE_META_PROP }),
  )
}

function importDeclarationSource(bindingPath) {
  return bindingPath.parentPath?.isImportDeclaration() ? bindingPath.parentPath.node.source.value : undefined
}

function isQtSolidCreateWindowCallee(calleePath, moduleName) {
  if (calleePath.isIdentifier()) {
    const binding = calleePath.scope.getBinding(calleePath.node.name)
    if (!binding?.path.isImportSpecifier()) {
      return false
    }

    const imported = binding.path.node.imported
    return importDeclarationSource(binding.path) === moduleName
      && imported.type === "Identifier"
      && imported.name === "createWindow"
  }

  if (!calleePath.isMemberExpression() || calleePath.node.computed) {
    return false
  }

  const propertyPath = calleePath.get("property")
  if (!propertyPath.isIdentifier({ name: "createWindow" })) {
    return false
  }

  const objectPath = calleePath.get("object")
  if (!objectPath.isIdentifier()) {
    return false
  }

  const binding = objectPath.scope.getBinding(objectPath.node.name)
  return Boolean(binding?.path.isImportNamespaceSpecifier() && importDeclarationSource(binding.path) === moduleName)
}

function qtSolidSourceMetadataPlugin({ types: t }) {
  return {
    name: "qt-solid-source-metadata",
    visitor: {
      Program: {
        enter(path, state) {
          state.qtSolidSourceMetadataHelperId = path.scope.generateUidIdentifier("qtSolidWithSourceMeta")
          state.qtSolidSourceMetadataHelperUsed = false
        },
        exit(path, state) {
          if (!state.qtSolidSourceMetadataHelperUsed) {
            return
          }

          path.unshiftContainer(
            "body",
            t.importDeclaration(
              [
                t.importSpecifier(
                  t.cloneNode(state.qtSolidSourceMetadataHelperId),
                  t.identifier("withQtSourceMeta"),
                ),
              ],
              t.stringLiteral(
                state.opts.compilerRuntimeModuleName ?? DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME,
              ),
            ),
          )
        },
      },
      JSXOpeningElement(path, state) {
        if (hasQtSolidSourceMetadataAttribute(t, path.node.attributes)) {
          return
        }

        const filename = state.file.opts.filename
        const location = path.node.loc?.start
        if (!filename || !location) {
          return
        }

        path.pushContainer(
          "attributes",
          t.jsxAttribute(
            t.jsxIdentifier(QT_SOLID_SOURCE_META_PROP),
            t.jsxExpressionContainer(
              createQtSolidSourceMetadataExpression(t, filename, location.line, location.column + 1),
            ),
          ),
        )
      },
      CallExpression(path, state) {
        if (!isQtSolidCreateWindowCallee(path.get("callee"), state.opts.publicModuleName ?? DEFAULT_QT_SOLID_MODULE_NAME)) {
          return
        }

        const [firstArgument] = path.node.arguments
        const filename = state.file.opts.filename
        const location = path.node.loc?.start
        if (!firstArgument || !filename || !location) {
          return
        }

        path.node.arguments[0] = t.callExpression(t.cloneNode(state.qtSolidSourceMetadataHelperId), [
          firstArgument,
          createQtSolidSourceMetadataExpression(t, filename, location.line, location.column + 1),
        ])
        state.qtSolidSourceMetadataHelperUsed = true
      },
    },
  }
}

export function shouldTransformQtSolidModule(filename) {
  return /\.[jt]sx$/.test(filename)
}

export function isQtSolidDependencyModule(filename) {
  return filename.includes("/node_modules/") || filename.includes("\\node_modules\\")
}

export async function transformQtSolidModule(code, options) {
  if (!shouldTransformQtSolidModule(options.filename) || isQtSolidDependencyModule(options.filename)) {
    return null
  }

  const hmrEnabled = options.hmr?.enabled ?? false
  const sourceFileName = normalizeQtSolidSourceFilename(options.filename)

  const publicModuleName = options.moduleName ?? DEFAULT_QT_SOLID_MODULE_NAME
  const compilerRuntimeModuleName =
    options.compilerRuntimeModuleName ?? DEFAULT_QT_SOLID_COMPILER_RT_MODULE_NAME

  const sourceMetadataInjected = await transformAsync(code, {
    filename: options.filename,
    sourceFileName,
    sourceMaps: options.sourceMaps ?? false,
    plugins: [[qtSolidSourceMetadataPlugin, { publicModuleName, compilerRuntimeModuleName }]],
    presets: [[ts]],
  })

  if (!sourceMetadataInjected?.code) {
    return null
  }

  const transformed = await transformAsync(sourceMetadataInjected.code, {
    filename: options.filename,
    sourceFileName,
    inputSourceMap: sourceMetadataInjected.map ?? undefined,
    sourceMaps: options.sourceMaps ?? false,
    plugins: hmrEnabled
      ? [
          [
            solidRefresh,
            {
              bundler: options.hmr?.bundler ?? "vite",
            },
          ],
        ]
      : [],
    presets: [
      [
        solid,
        {
          moduleName: compilerRuntimeModuleName,
          generate: "universal",
        },
      ],
    ],
  })

  if (!transformed?.code) {
    return null
  }

  return {
    code: transformed.code,
    map: transformed.map ?? null,
  }
}
