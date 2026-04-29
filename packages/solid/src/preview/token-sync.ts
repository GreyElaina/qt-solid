// ---------------------------------------------------------------------------
// Design Token sync — bidirectional between qt-solid theme files and Figma Variables
//
// Code → Figma: parse FluentTokens interface + value objects → Figma Variable format
// Figma → Code: Figma Variables → generate theme.ts source
// ---------------------------------------------------------------------------

import ts from "typescript"
import { readFile, writeFile } from "node:fs/promises"

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

export interface DesignToken {
  name: string
  type: "COLOR" | "FLOAT" | "STRING" | "BOOLEAN"
  values: Record<string, unknown>  // mode name → value
  collection?: string
}

// ---------------------------------------------------------------------------
// Code → Figma: extract tokens from theme source file
// ---------------------------------------------------------------------------

export interface ExtractedTheme {
  interfaceName: string
  tokens: DesignToken[]
  modes: string[]
}

export function extractThemeTokens(filePath: string): ExtractedTheme {
  const program = ts.createProgram([filePath], {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    strict: false,
    skipLibCheck: true,
    noEmit: true,
  })

  const checker = program.getTypeChecker()
  const sourceFile = program.getSourceFile(filePath)
  if (!sourceFile) return { interfaceName: "", tokens: [], modes: [] }

  // Find the tokens interface (convention: ends with "Tokens")
  let tokensInterface: ts.InterfaceDeclaration | undefined
  ts.forEachChild(sourceFile, (node) => {
    if (ts.isInterfaceDeclaration(node) && node.name.text.endsWith("Tokens")) {
      tokensInterface = node
    }
  })

  if (!tokensInterface) return { interfaceName: "", tokens: [], modes: [] }

  // Find all exported const objects that satisfy the interface
  const modeObjects: Array<{ name: string; values: Record<string, unknown> }> = []
  ts.forEachChild(sourceFile, (node) => {
    if (!ts.isVariableStatement(node)) return
    for (const decl of node.declarationList.declarations) {
      if (!ts.isIdentifier(decl.name) || !decl.initializer) continue
      if (!ts.isObjectLiteralExpression(decl.initializer)) continue

      // Check if this variable's type matches the tokens interface
      const type = checker.getTypeAtLocation(decl)
      const typeSymbol = type.getSymbol() ?? type.aliasSymbol
      if (typeSymbol?.getName() === tokensInterface!.name.text ||
          checker.isTypeAssignableTo(type, checker.getTypeAtLocation(tokensInterface!))) {
        const values = extractObjectLiteral(decl.initializer)
        modeObjects.push({ name: decl.name.text, values })
      }
    }
  })

  // Build token list from interface fields + mode values
  const interfaceType = checker.getTypeAtLocation(tokensInterface)
  const tokens: DesignToken[] = []

  for (const prop of interfaceType.getProperties()) {
    const name = prop.getName()
    const propType = checker.getTypeOfSymbol(prop)
    const tokenType = inferTokenType(checker, propType, name)

    const values: Record<string, unknown> = {}
    for (const mode of modeObjects) {
      if (name in mode.values) {
        values[mode.name] = mode.values[name]
      }
    }

    tokens.push({ name, type: tokenType, values })
  }

  return {
    interfaceName: tokensInterface.name.text,
    tokens,
    modes: modeObjects.map(m => m.name),
  }
}

function inferTokenType(checker: ts.TypeChecker, type: ts.Type, name: string): DesignToken["type"] {
  if (type.flags & ts.TypeFlags.Boolean || type.flags & ts.TypeFlags.BooleanLiteral) return "BOOLEAN"
  if (type.flags & ts.TypeFlags.Number || type.flags & ts.TypeFlags.NumberLiteral) return "FLOAT"
  if (type.flags & ts.TypeFlags.String || type.flags & ts.TypeFlags.StringLiteral) {
    // Heuristic: color tokens have color-ish names or hex values
    const lower = name.toLowerCase()
    if (lower.includes("color") || lower.includes("accent") || lower.includes("foreground") ||
        lower.includes("background") || lower.includes("stroke") || lower.includes("control") ||
        lower.includes("focus") || lower.includes("fill")) {
      return "COLOR"
    }
    return "STRING"
  }
  return "STRING"
}

function extractObjectLiteral(expr: ts.ObjectLiteralExpression): Record<string, unknown> {
  const result: Record<string, unknown> = {}
  for (const prop of expr.properties) {
    if (!ts.isPropertyAssignment(prop) || !ts.isIdentifier(prop.name)) continue
    const name = prop.name.text
    const init = prop.initializer
    if (ts.isStringLiteral(init)) {
      result[name] = init.text
    } else if (ts.isNumericLiteral(init)) {
      result[name] = Number(init.text)
    } else if (init.kind === ts.SyntaxKind.TrueKeyword) {
      result[name] = true
    } else if (init.kind === ts.SyntaxKind.FalseKeyword) {
      result[name] = false
    }
  }
  return result
}

// ---------------------------------------------------------------------------
// Figma → Code: generate theme source from Figma-exported tokens
// ---------------------------------------------------------------------------

export interface GenerateThemeOptions {
  interfaceName: string
  tokens: DesignToken[]
  modes: Array<{ variableName: string; modeName: string }>
}

export function generateThemeSource(options: GenerateThemeOptions): string {
  const { interfaceName, tokens, modes } = options
  const lines: string[] = []

  lines.push(`import { createContext, useContext, type Accessor } from "solid-js"`)
  lines.push("")

  // Interface
  lines.push(`export interface ${interfaceName} {`)
  for (const token of tokens) {
    const tsType = token.type === "COLOR" || token.type === "STRING" ? "string"
      : token.type === "FLOAT" ? "number"
      : token.type === "BOOLEAN" ? "boolean"
      : "string"
    lines.push(`  readonly ${token.name}: ${tsType}`)
  }
  lines.push(`}`)
  lines.push("")

  // Mode objects
  for (const mode of modes) {
    lines.push(`export const ${mode.variableName}: ${interfaceName} = {`)
    for (const token of tokens) {
      const value = token.values[mode.modeName]
      if (value === undefined) continue
      if (typeof value === "string") {
        lines.push(`  ${token.name}: ${JSON.stringify(value)},`)
      } else if (typeof value === "number") {
        lines.push(`  ${token.name}: ${value},`)
      } else if (typeof value === "boolean") {
        lines.push(`  ${token.name}: ${value},`)
      }
    }
    lines.push(`}`)
    lines.push("")
  }

  // Context
  const defaultMode = modes[0]
  if (defaultMode) {
    lines.push(`const ThemeContext = createContext<Accessor<${interfaceName}>>(() => ${defaultMode.variableName})`)
    lines.push("")
    lines.push(`export const ThemeProvider = ThemeContext.Provider`)
    lines.push("")
    lines.push(`export function useTheme(): Accessor<${interfaceName}> {`)
    lines.push(`  return useContext(ThemeContext)`)
    lines.push(`}`)
  }

  return lines.join("\n") + "\n"
}

// ---------------------------------------------------------------------------
// Parse Figma-exported token JSON → DesignToken[]
// ---------------------------------------------------------------------------

export interface FigmaTokenExport {
  collection: string
  name: string
  type: string
  values: Record<string, unknown>
}

export function parseFigmaTokens(raw: FigmaTokenExport[]): DesignToken[] {
  return raw.map((t) => ({
    name: tokenNameToIdentifier(t.name),
    type: t.type as DesignToken["type"],
    values: normalizeFigmaValues(t.values),
    collection: t.collection,
  }))
}

function tokenNameToIdentifier(name: string): string {
  // Figma uses "/" separators → camelCase
  return name
    .split("/")
    .map((seg, i) => {
      const clean = seg.replace(/[^a-zA-Z0-9]/g, "")
      if (i === 0) return clean[0]!.toLowerCase() + clean.slice(1)
      return clean[0]!.toUpperCase() + clean.slice(1)
    })
    .join("")
}

function normalizeFigmaValues(values: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {}
  for (const [mode, val] of Object.entries(values)) {
    if (typeof val === "object" && val !== null && "r" in val) {
      // RGBA → hex string
      const c = val as { r: number; g: number; b: number; a?: number }
      const toHex = (v: number) => Math.round(v * 255).toString(16).padStart(2, "0")
      const a = c.a ?? 1
      result[mode] = a >= 0.999
        ? `#${toHex(c.r)}${toHex(c.g)}${toHex(c.b)}`
        : `#${toHex(c.r)}${toHex(c.g)}${toHex(c.b)}${toHex(a)}`
    } else {
      result[mode] = val
    }
  }
  return result
}

// ---------------------------------------------------------------------------
// High-level sync operations
// ---------------------------------------------------------------------------

/**
 * Read a theme.ts file and produce Figma-compatible token list.
 * Used by: Code → Figma direction.
 */
export function codeToFigmaTokens(filePath: string): {
  tokens: Array<{
    name: string
    type: "COLOR" | "FLOAT" | "STRING" | "BOOLEAN"
    value: unknown
    collectionName: string
  }>
  modes: string[]
} {
  const theme = extractThemeTokens(filePath)
  if (theme.tokens.length === 0) return { tokens: [], modes: [] }

  // Flatten: for each token × mode → one Figma write entry
  // Use first mode as default
  const firstMode = theme.modes[0]
  if (!firstMode) return { tokens: [], modes: [] }

  const tokens = theme.tokens
    .filter(t => t.values[firstMode] !== undefined)
    .map(t => ({
      name: t.name,
      type: t.type,
      value: t.type === "COLOR" ? hexToFigmaColor(t.values[firstMode] as string) : t.values[firstMode],
      collectionName: theme.interfaceName.replace("Tokens", " Tokens"),
    }))

  return { tokens, modes: theme.modes }
}

function hexToFigmaColor(hex: string): { r: number; g: number; b: number; a: number } {
  const clean = hex.replace("#", "")
  const r = parseInt(clean.slice(0, 2), 16) / 255
  const g = parseInt(clean.slice(2, 4), 16) / 255
  const b = parseInt(clean.slice(4, 6), 16) / 255
  const a = clean.length > 6 ? parseInt(clean.slice(6, 8), 16) / 255 : 1
  return { r, g, b, a }
}

/**
 * Take Figma-exported tokens and write/update a theme.ts file.
 * Used by: Figma → Code direction.
 */
export async function figmaToCodeTheme(
  figmaTokens: FigmaTokenExport[],
  outputPath: string,
  options: {
    interfaceName?: string
    modes?: Array<{ variableName: string; modeName: string }>
  } = {},
): Promise<void> {
  const tokens = parseFigmaTokens(figmaTokens)
  const interfaceName = options.interfaceName ?? "ThemeTokens"

  // Infer modes from token values
  const allModes = new Set<string>()
  for (const t of tokens) {
    for (const mode of Object.keys(t.values)) {
      allModes.add(mode)
    }
  }

  const modes = options.modes ?? [...allModes].map((m) => ({
    variableName: `theme${m[0]!.toUpperCase()}${m.slice(1)}`,
    modeName: m,
  }))

  const source = generateThemeSource({ interfaceName, tokens, modes })
  await writeFile(outputPath, source, "utf-8")
}
