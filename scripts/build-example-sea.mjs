import { resolve } from "node:path"
import { fileURLToPath } from "node:url"

import {
  buildSolidNodeSeaExecutable,
} from "../packages/solid/src/build/build-solid-node-sea.js"
import { ensureSeaNodeBinary } from "./ensure-sea-node.mjs"

const projectRoot = fileURLToPath(new URL("..", import.meta.url))

const exampleName = process.argv[2]
const targetText = process.argv[3] ?? process.env.QT_SOLID_SEA_TARGET ?? null

function parseTarget(target) {
  if (!target) {
    return { platform: process.platform, arch: process.arch }
  }

  const normalized = target.replace(/^windows-/, "win32-")
  const [platform, arch] = normalized.split("-", 2)
  if (!platform || !arch) {
    throw new Error(`Unsupported SEA target ${target}; expected <platform>-<arch> like win32-x64`)
  }
  return { platform, arch }
}

const target = parseTarget(targetText)
const targetSuffix =
  target.platform === process.platform && target.arch === process.arch
    ? "dist-sea"
    : `dist-sea-${target.platform}-${target.arch}`

const exampleConfigs = {
  counter: {
    entryPath: resolve(projectRoot, "examples/counter/app.tsx"),
    outDir: resolve(projectRoot, "examples/counter", targetSuffix),
  },
  "spin-triangle": {
    entryPath: resolve(projectRoot, "examples/spin-triangle/app.tsx"),
    outDir: resolve(projectRoot, "examples/spin-triangle", targetSuffix),
    widgetLibraries: [
      "@qt-solid/core-widgets/widget-library",
      "@qt-solid/example-widgets/widget-library",
    ],
  },
}

if (!exampleName || !(exampleName in exampleConfigs)) {
  throw new Error(
    `Unsupported qt-solid SEA example ${exampleName ?? "<missing>"}. Expected one of: ${Object.keys(exampleConfigs).join(", ")}`,
  )
}

const prepNodeBinary = process.env.QT_SOLID_SEA_PREP_NODE_BINARY || process.execPath
const targetNodeBinary = process.env.QT_SOLID_SEA_NODE_BINARY || await ensureSeaNodeBinary({
  version: process.env.QT_SOLID_SEA_NODE_VERSION,
  platform: target.platform,
  arch: target.arch,
})

const result = await buildSolidNodeSeaExecutable({
  ...exampleConfigs[exampleName],
  prepNodeBinary,
  targetNodeBinary,
  targetPlatform: target.platform,
  targetArch: target.arch,
})

console.log(`qt-solid SEA prep blob: ${result.prepBlobPath}`)
console.log(`qt-solid SEA config: ${result.seaConfigPath}`)
console.log(`qt-solid SEA prep node: ${prepNodeBinary}`)
console.log(`qt-solid SEA target node: ${targetNodeBinary}`)
console.log(`qt-solid SEA target: ${target.platform}/${target.arch}`)
if (result.executablePath) {
  console.log(`qt-solid SEA executable: ${result.executablePath}`)
} else if (result.capabilityError) {
  console.log(`qt-solid SEA executable unavailable: ${result.capabilityError}`)
}
