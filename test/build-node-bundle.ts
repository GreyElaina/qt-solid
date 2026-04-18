import { rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"

import { buildSolidNodeBundle } from "../packages/solid/src/build/build-solid-node-bundle.js"

export interface BuildNodeBundleOptions {
  bootstrap?: boolean
  entryExtension: ".ts" | ".tsx"
  entrySource: string
  projectRoot: string
  tag: string
  widgetLibraries?: readonly string[]
}

export async function buildNodeBundle(
  options: BuildNodeBundleOptions,
): Promise<{ bundlePath: string; cleanup: () => void }> {
  const entryPath = join(options.projectRoot, `${options.tag}${options.entryExtension}`)
  const bundlePath = join(options.projectRoot, `${options.tag}.mjs`)

  writeFileSync(entryPath, options.entrySource)

  const cleanup = () => {
    rmSync(entryPath, { force: true })
    rmSync(bundlePath, { force: true })
    rmSync(`${bundlePath}.map`, { force: true })
  }

  try {
    await buildSolidNodeBundle({
      bootstrap: options.bootstrap,
      entryPath,
      outfile: bundlePath,
      widgetLibraries: options.widgetLibraries,
    })

    return { bundlePath, cleanup }
  } catch (error) {
    cleanup()
    throw error
  }
}
