// ---------------------------------------------------------------------------
// Extract props schema from component source using the TypeScript compiler API.
//
// Given a file path and export name, resolves the component's props type
// and emits PropFieldSchema + VariantAxisSchema entries.
// ---------------------------------------------------------------------------

import ts from "typescript"
import type { PropFieldSchema, VariantAxisSchema } from "./protocol.ts"

export interface ExtractedPropsSchema {
  props: PropFieldSchema[]
  variantAxes: VariantAxisSchema[]
}

export function extractPropsSchema(
  filePath: string,
  exportName: string,
): ExtractedPropsSchema {
  const program = ts.createProgram([filePath], {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    jsx: ts.JsxEmit.Preserve,
    strict: false,
    skipLibCheck: true,
    noEmit: true,
  })

  const checker = program.getTypeChecker()
  const sourceFile = program.getSourceFile(filePath)
  if (!sourceFile) return { props: [], variantAxes: [] }

  const symbol = findExportSymbol(checker, sourceFile, exportName)
  if (!symbol) return { props: [], variantAxes: [] }

  const componentType = checker.getTypeOfSymbol(symbol)
  const propsType = resolveComponentPropsType(checker, componentType)
  if (!propsType) return { props: [], variantAxes: [] }

  const props = extractFieldsFromType(checker, propsType)
  const variantAxes = extractVariantAxes(checker, sourceFile, exportName)

  return { props, variantAxes }
}

function findExportSymbol(
  checker: ts.TypeChecker,
  sourceFile: ts.SourceFile,
  exportName: string,
): ts.Symbol | undefined {
  const moduleSymbol = checker.getSymbolAtLocation(sourceFile)
  if (!moduleSymbol) return undefined
  const exports = checker.getExportsOfModule(moduleSymbol)
  if (exportName === "default") {
    return exports.find(s => s.escapedName === "default")
  }
  return exports.find(s => s.escapedName === exportName)
}

function resolveComponentPropsType(
  checker: ts.TypeChecker,
  componentType: ts.Type,
): ts.Type | undefined {
  // Component<P> — look for call signatures (P) => JSX.Element
  const signatures = componentType.getCallSignatures()
  if (signatures.length > 0) {
    const sig = signatures[0]!
    const params = sig.getParameters()
    if (params.length > 0) {
      return checker.getTypeOfSymbol(params[0]!)
    }
  }

  // createVariants result — also a component, check call signatures of resolved type
  if (componentType.isUnionOrIntersection()) {
    for (const t of componentType.types) {
      const sigs = t.getCallSignatures()
      if (sigs.length > 0) {
        const sig = sigs[0]!
        const params = sig.getParameters()
        if (params.length > 0) {
          return checker.getTypeOfSymbol(params[0]!)
        }
      }
    }
  }

  return undefined
}

function extractFieldsFromType(
  checker: ts.TypeChecker,
  propsType: ts.Type,
): PropFieldSchema[] {
  const fields: PropFieldSchema[] = []

  for (const prop of propsType.getProperties()) {
    const name = prop.getName()
    // Skip event handlers and children
    if (name.startsWith("on") && name.length > 2 && name[2] === name[2]!.toUpperCase()) continue
    if (name === "children" || name === "ref") continue

    const propType = checker.getTypeOfSymbol(prop)
    const field = typeToFieldSchema(checker, name, propType)
    if (field) fields.push(field)
  }

  return fields
}

function typeToFieldSchema(
  checker: ts.TypeChecker,
  name: string,
  type: ts.Type,
): PropFieldSchema | null {
  // Unwrap optional (T | undefined)
  if (type.isUnion()) {
    const nonUndefined = type.types.filter(t => !(t.flags & ts.TypeFlags.Undefined))
    if (nonUndefined.length === 0) return null
    if (nonUndefined.length === 1) {
      return typeToFieldSchema(checker, name, nonUndefined[0]!)
    }
    // Check if all are string literals → enum
    if (nonUndefined.every(t => t.isStringLiteral())) {
      return {
        name,
        type: "enum",
        values: nonUndefined.map(t => (t as ts.StringLiteralType).value),
      }
    }
    // Mixed union — fall back to string
    return { name, type: "string" }
  }

  if (type.flags & ts.TypeFlags.Boolean || type.flags & ts.TypeFlags.BooleanLiteral) {
    return { name, type: "boolean", defaultValue: false }
  }
  if (type.flags & ts.TypeFlags.Number || type.flags & ts.TypeFlags.NumberLiteral) {
    return { name, type: "number" }
  }
  if (type.flags & ts.TypeFlags.String || type.flags & ts.TypeFlags.StringLiteral) {
    // Heuristic: if name contains "color" or "fill" or "stroke", mark as color
    const lower = name.toLowerCase()
    if (lower.includes("color") || lower === "fill" || lower === "stroke") {
      return { name, type: "color" }
    }
    return { name, type: "string" }
  }

  return null
}

// ---------------------------------------------------------------------------
// Variant axis extraction — scan for createVariants({ variants: { ... } })
// in the source AST
// ---------------------------------------------------------------------------

function extractVariantAxes(
  checker: ts.TypeChecker,
  sourceFile: ts.SourceFile,
  exportName: string,
): VariantAxisSchema[] {
  const axes: VariantAxisSchema[] = []

  function visit(node: ts.Node): void {
    if (
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "createVariants" &&
      node.arguments.length >= 2
    ) {
      // Check if this createVariants is assigned to our export
      const parent = node.parent
      const isOurExport =
        ts.isVariableDeclaration(parent) &&
        ts.isIdentifier(parent.name) &&
        parent.name.text === exportName

      if (!isOurExport) {
        ts.forEachChild(node, visit)
        return
      }

      const configArg = node.arguments[1]
      if (!configArg || !ts.isObjectLiteralExpression(configArg)) return

      for (const prop of configArg.properties) {
        if (!ts.isPropertyAssignment(prop)) continue
        if (!ts.isIdentifier(prop.name) || prop.name.text !== "variants") continue
        if (!ts.isObjectLiteralExpression(prop.initializer)) continue

        for (const axisProp of prop.initializer.properties) {
          if (!ts.isPropertyAssignment(axisProp)) continue
          if (!ts.isIdentifier(axisProp.name)) continue
          if (!ts.isObjectLiteralExpression(axisProp.initializer)) continue

          const axisName = axisProp.name.text
          const values: string[] = []
          for (const valueProp of axisProp.initializer.properties) {
            if (ts.isPropertyAssignment(valueProp) && ts.isIdentifier(valueProp.name)) {
              values.push(valueProp.name.text)
            }
          }

          if (values.length > 0) {
            axes.push({ name: axisName, values })
          }
        }
      }

      // Also extract defaultVariants
      for (const prop of configArg.properties) {
        if (!ts.isPropertyAssignment(prop)) continue
        if (!ts.isIdentifier(prop.name) || prop.name.text !== "defaultVariants") continue
        if (!ts.isObjectLiteralExpression(prop.initializer)) continue

        for (const dvProp of prop.initializer.properties) {
          if (!ts.isPropertyAssignment(dvProp)) continue
          if (!ts.isIdentifier(dvProp.name)) continue
          const axis = axes.find(a => a.name === dvProp.name!.getText())
          if (axis && ts.isStringLiteral(dvProp.initializer)) {
            axis.defaultValue = dvProp.initializer.text
          }
        }
      }
    }

    ts.forEachChild(node, visit)
  }

  visit(sourceFile)
  return axes
}
