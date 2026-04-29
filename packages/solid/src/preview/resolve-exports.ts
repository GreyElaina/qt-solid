import { readFile } from "node:fs/promises"

export interface ExportInfo {
  name: string
  /** "default" | "named" */
  kind: "default" | "named"
  /** Whether this looks like a component (starts with uppercase) */
  isComponent: boolean
  /** Whether this is a createVariants() call */
  isVariant: boolean
}

const PATTERNS = {
  exportFunction: /export\s+function\s+(\w+)/g,
  exportConst: /export\s+(?:const|let)\s+(\w+)/g,
  exportDefault: /export\s+default\s+(?:function\s+)?(\w+)?/g,
  exportNamed: /export\s*\{([^}]+)\}/g,
  createVariants: /(\w+)\s*=\s*createVariants\s*\(/g,
  createApp: /(\w+)\s*=\s*createApp\s*\(/g,
}

function isUpperCase(name: string): boolean {
  return /^[A-Z]/.test(name)
}

function extractExports(source: string): ExportInfo[] {
  const results: ExportInfo[] = []
  const seen = new Set<string>()

  const variantNames = new Set<string>()
  for (const m of source.matchAll(PATTERNS.createVariants)) {
    if (m[1]) variantNames.add(m[1])
  }
  for (const m of source.matchAll(PATTERNS.createApp)) {
    if (m[1]) variantNames.add(m[1])
  }

  function add(name: string, kind: "default" | "named") {
    if (seen.has(name)) return
    seen.add(name)
    const isComponent = isUpperCase(name)
    const isVariant = variantNames.has(name)
    if (isComponent) {
      results.push({ name, kind, isComponent, isVariant })
    }
  }

  for (const m of source.matchAll(PATTERNS.exportFunction)) {
    if (m[1]) add(m[1], "named")
  }

  for (const m of source.matchAll(PATTERNS.exportConst)) {
    if (m[1]) add(m[1], "named")
  }

  for (const m of source.matchAll(PATTERNS.exportDefault)) {
    const name = m[1] ?? "default"
    add(name, "default")
  }

  for (const m of source.matchAll(PATTERNS.exportNamed)) {
    const inner = m[1]
    if (!inner) continue
    for (const part of inner.split(",")) {
      // handle `X as Y` — use the exported name (Y)
      const segments = part.trim().split(/\s+as\s+/)
      const exportedName = (segments[segments.length - 1] ?? "").trim()
      if (exportedName) {
        add(exportedName, "named")
      }
    }
  }

  return results
}

/**
 * Scan a source file and return its exports that appear to be components.
 * This is a best-effort heuristic, not a full parse.
 */
export async function resolvePreviewableExports(
  filePath: string,
): Promise<ExportInfo[]> {
  const source = await readFile(filePath, "utf-8")
  return extractExports(source)
}
