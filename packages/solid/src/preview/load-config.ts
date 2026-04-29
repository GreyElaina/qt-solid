// ---------------------------------------------------------------------------
// Resolve qt-solid.config.ts from project root
// ---------------------------------------------------------------------------

import { existsSync } from "node:fs"
import { resolve, join } from "node:path"
import { pathToFileURL } from "node:url"
import type { QtSolidConfig } from "./config.ts"

const CONFIG_NAMES = [
  "qt-solid.config.ts",
  "qt-solid.config.js",
  "qt-solid.config.mjs",
]

export async function loadConfig(cwd?: string): Promise<QtSolidConfig> {
  const root = cwd ?? process.cwd()

  for (const name of CONFIG_NAMES) {
    const configPath = join(root, name)
    if (!existsSync(configPath)) continue

    const mod = await import(pathToFileURL(configPath).href)
    const config: QtSolidConfig = mod.default ?? mod
    return resolveConfigPaths(config, root)
  }

  return {}
}

function resolveConfigPaths(config: QtSolidConfig, root: string): QtSolidConfig {
  const resolved = { ...config }

  if (resolved.theme) {
    resolved.theme = resolve(root, resolved.theme)
  }

  if (resolved.components) {
    if (typeof resolved.components === "string") {
      resolved.components = resolve(root, resolved.components)
    } else {
      resolved.components = resolved.components.map(c => resolve(root, c))
    }
  }

  return resolved
}
