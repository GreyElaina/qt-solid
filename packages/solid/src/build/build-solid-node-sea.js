import { spawnSync } from "node:child_process"
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import ts from "typescript"

import { buildSolidNodeBundle } from "./build-solid-node-bundle.js"

const repositoryRoot = fileURLToPath(new URL("../../../../", import.meta.url))
const defaultWorkerAssetPath = fileURLToPath(new URL("../devtools/cdp-worker.mjs", import.meta.url))
const assetRuntimeExtensions = new Set([".js", ".mjs", ".ts", ".json", ".node"])
const seaSentinel = "NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2"
const directSeaMinimumVersion = [25, 5, 0]

const runtimePackageSpecs = {
  "@qt-solid/core": {
    packageDir: resolve(repositoryRoot, "packages/core"),
    entries: ["package.json", "native", "lowlevel"],
  },
}

const runtimePackageNames = Object.keys(runtimePackageSpecs)

function nativeBinaryCandidates(platform, arch) {
  if (platform === "darwin") {
    return [`index.darwin-${arch}.node`, "index.darwin-universal.node"]
  }

  if (platform === "linux") {
    if (arch === "x64") {
      return ["index.linux-x64-gnu.node", "index.linux-x64-musl.node"]
    }
    if (arch === "arm64") {
      return ["index.linux-arm64-gnu.node", "index.linux-arm64-musl.node"]
    }
    if (arch === "arm") {
      return ["index.linux-arm-gnueabihf.node", "index.linux-arm-musleabihf.node"]
    }
    return []
  }

  if (platform === "win32") {
    if (arch === "x64") {
      return ["index.win32-x64-msvc.node", "index.win32-x64-gnu.node"]
    }
    if (arch === "arm64") {
      return ["index.win32-arm64-msvc.node"]
    }
    if (arch === "ia32") {
      return ["index.win32-ia32-msvc.node"]
    }
    return []
  }

  return []
}

function ensureDir(path) {
  mkdirSync(path, { recursive: true })
}

function fileExtension(path) {
  const index = path.lastIndexOf(".")
  return index === -1 ? "" : path.slice(index)
}

function isDeclarationFile(path) {
  return path.endsWith(".d.ts")
}

function isTypeScriptRuntimeFile(path) {
  return path.endsWith(".ts") && !isDeclarationFile(path)
}

function shouldCopyRuntimeFile(path) {
  if (isDeclarationFile(path)) {
    return false
  }
  return assetRuntimeExtensions.has(fileExtension(path))
}

function rewriteRuntimeSpecifier(specifier) {
  if (typeof specifier !== "string") {
    return specifier
  }

  if (specifier.endsWith(".d.ts")) {
    return specifier
  }

  if (specifier.endsWith(".ts")) {
    return `${specifier.slice(0, -3)}.js`
  }

  return specifier
}

function rewritePackageExports(value, currentKey = null) {
  if (typeof value === "string") {
    return currentKey === "types" ? value : rewriteRuntimeSpecifier(value)
  }

  if (Array.isArray(value)) {
    return value.map((entry) => rewritePackageExports(entry, currentKey))
  }

  if (!value || typeof value !== "object") {
    return value
  }

  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [key, rewritePackageExports(entry, key)]),
  )
}

function rewriteRuntimePackageManifest(sourcePath, targetPath) {
  const manifest = JSON.parse(readFileSync(sourcePath, "utf8"))
  if (typeof manifest.main === "string") {
    manifest.main = rewriteRuntimeSpecifier(manifest.main)
  }
  if (typeof manifest.module === "string") {
    manifest.module = rewriteRuntimeSpecifier(manifest.module)
  }
  if (manifest.exports !== undefined) {
    manifest.exports = rewritePackageExports(manifest.exports)
  }

  ensureDir(dirname(targetPath))
  writeFileSync(targetPath, `${JSON.stringify(manifest, null, 2)}\n`)
}

function transpileTypeScriptRuntimeFile(sourcePath, targetPath) {
  const sourceText = readFileSync(sourcePath, "utf8")
  const result = ts.transpileModule(sourceText, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      verbatimModuleSyntax: true,
      rewriteRelativeImportExtensions: true,
      sourceMap: false,
    },
    fileName: sourcePath,
  })

  ensureDir(dirname(targetPath))
  writeFileSync(targetPath, result.outputText)
}

function runtimeTargetPath(sourcePath, targetPath) {
  return isTypeScriptRuntimeFile(sourcePath) ? `${targetPath.slice(0, -3)}.js` : targetPath
}

function copyRuntimeTree(sourcePath, targetPath) {
  const stats = statSync(sourcePath)
  if (stats.isDirectory()) {
    ensureDir(targetPath)
    for (const entry of readdirSync(sourcePath)) {
      copyRuntimeTree(join(sourcePath, entry), join(targetPath, entry))
    }
    return
  }

  if (!shouldCopyRuntimeFile(sourcePath)) {
    return
  }

  const resolvedTargetPath = runtimeTargetPath(sourcePath, targetPath)
  if (sourcePath.endsWith("package.json")) {
    rewriteRuntimePackageManifest(sourcePath, resolvedTargetPath)
    return
  }

  if (isTypeScriptRuntimeFile(sourcePath)) {
    transpileTypeScriptRuntimeFile(sourcePath, resolvedTargetPath)
    return
  }

  ensureDir(dirname(resolvedTargetPath))
  copyFileSync(sourcePath, resolvedTargetPath)
}

function validateNativeRuntimePackages(packageNames, targetPlatform, targetArch) {
  for (const packageName of packageNames) {
    const spec = runtimePackageSpecs[packageName]
    if (!spec || !spec.entries.includes("native")) {
      continue
    }

    const candidates = nativeBinaryCandidates(targetPlatform, targetArch)
    if (candidates.length === 0) {
      throw new Error(`qt-solid SEA does not know native addon naming for ${targetPlatform}/${targetArch}`)
    }

    const nativeDir = join(spec.packageDir, "native")
    const match = candidates.find((candidate) => existsSync(join(nativeDir, candidate)))
    if (!match) {
      throw new Error(
        `qt-solid SEA missing native addon for ${packageName} on ${targetPlatform}/${targetArch}; expected one of ${candidates.join(", ")} under ${nativeDir}`,
      )
    }
  }
}

function stageRuntimePackage(packageName, stagingRoot) {
  const spec = runtimePackageSpecs[packageName]
  if (!spec) {
    throw new Error(`qt-solid SEA does not know how to stage runtime package ${packageName}`)
  }

  const packageRoot = join(stagingRoot, "node_modules", packageName)
  for (const entry of spec.entries) {
    copyRuntimeTree(join(spec.packageDir, entry), join(packageRoot, entry))
  }
}

function stageSeaAsset(assetMap, assetKey, sourcePath) {
  if (!existsSync(sourcePath)) {
    throw new Error(`qt-solid SEA asset missing: ${sourcePath}`)
  }
  assetMap[assetKey] = sourcePath
}

function writeInjectedMain(path) {
  writeFileSync(
    path,
    [
      "const sea = require('node:sea')",
      "const { existsSync, mkdtempSync, mkdirSync, writeFileSync } = require('node:fs')",
      "const { createRequire } = require('node:module')",
      "const { tmpdir } = require('node:os')",
      "const { dirname, join } = require('node:path')",
      "",
      "if (!sea.isSea()) {",
      "  throw new Error('qt-solid SEA entry must run inside single executable application')",
      "}",
      "",
      "const sidecarRoot = dirname(process.execPath)",
      "const hasSidecarRuntime = existsSync(join(sidecarRoot, 'app', 'app.mjs')) && existsSync(join(sidecarRoot, 'node_modules'))",
      "const root = hasSidecarRuntime ? sidecarRoot : mkdtempSync(join(tmpdir(), 'qt-solid-sea-'))",
      "process.env.QT_SOLID_SEA_ROOT = root",
      "if (!hasSidecarRuntime) {",
      "  for (const key of sea.getAssetKeys()) {",
      "    const targetPath = join(root, ...key.split('/'))",
      "    mkdirSync(dirname(targetPath), { recursive: true })",
      "    writeFileSync(targetPath, Buffer.from(sea.getAsset(key)))",
      "  }",
      "}",
      "process.chdir(root)",
      "const bootstrapPath = join(root, '__sea_bootstrap__.cjs')",
      "writeFileSync(",
      "  bootstrapPath,",
      "  [",
      "    \"const { pathToFileURL } = require('node:url')\",",
      "    `const entryUrl = pathToFileURL(${JSON.stringify(join(root, 'app', 'app.mjs'))}).href`,",
      "    'void import(entryUrl)',",
      "  ].join('\\n'),",
      ")",
      "createRequire(bootstrapPath)(bootstrapPath)",
    ].join("\n"),
  )
}

function writeSeaConfig(path, config) {
  writeFileSync(path, `${JSON.stringify(config, null, 2)}\n`)
}

function runSeaPreparation(nodeBinary, seaConfigPath, cwd) {
  const result = spawnSync(nodeBinary, ["--experimental-sea-config", seaConfigPath], {
    cwd,
    encoding: "utf8",
  })
  if (result.status !== 0 || result.signal || result.error) {
    throw new Error(
      [
        "qt-solid SEA preparation failed",
        result.error ? String(result.error) : null,
        result.stderr?.trim() || null,
      ]
        .filter(Boolean)
        .join(": "),
    )
  }
}

function parseNodeVersion(versionText) {
  const match = /^v?(\d+)\.(\d+)\.(\d+)/.exec(versionText.trim())
  if (!match) {
    throw new Error(`Unable to parse Node version from ${JSON.stringify(versionText)}`)
  }

  return match.slice(1).map((part) => Number.parseInt(part, 10))
}

function compareSemver(left, right) {
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const leftPart = left[index] ?? 0
    const rightPart = right[index] ?? 0
    if (leftPart !== rightPart) {
      return leftPart - rightPart
    }
  }
  return 0
}

function nodeBinaryVersion(nodeBinary) {
  const result = spawnSync(nodeBinary, ["--version"], {
    encoding: "utf8",
  })
  if (result.status !== 0 || result.signal || result.error) {
    throw new Error(
      [
        `Failed to read Node version from ${nodeBinary}`,
        result.error ? String(result.error) : null,
        result.stderr?.trim() || null,
      ]
        .filter(Boolean)
        .join(": "),
    )
  }

  return parseNodeVersion(result.stdout)
}

function supportsDirectSeaBuild(nodeBinary) {
  return compareSemver(nodeBinaryVersion(nodeBinary), directSeaMinimumVersion) >= 0
}

function binaryHasSeaFuse(executablePath) {
  if (!existsSync(executablePath)) {
    return false
  }

  return readFileSync(executablePath).includes(Buffer.from(seaSentinel))
}

function signMacosSeaBinary(executablePath) {
  const result = spawnSync("codesign", ["--sign", "-", executablePath], {
    encoding: "utf8",
  })
  if (result.status !== 0 || result.signal || result.error) {
    throw new Error(
      [
        "qt-solid SEA codesign failed",
        result.error ? String(result.error) : null,
        result.stderr?.trim() || null,
      ]
        .filter(Boolean)
        .join(": "),
    )
  }
}

function removeMacosSignature(executablePath) {
  const result = spawnSync("codesign", ["--remove-signature", executablePath], {
    encoding: "utf8",
  })
  if (result.status === 0 && !result.signal && !result.error) {
    return
  }

  const stderr = result.stderr?.trim() ?? ""
  if (stderr.includes("not signed") || stderr.includes("is already unsigned")) {
    return
  }

  throw new Error(
    [
      `qt-solid SEA failed to remove existing macOS signature from ${executablePath}`,
      result.error ? String(result.error) : null,
      stderr || null,
    ]
      .filter(Boolean)
      .join(": "),
  )
}

function removeWindowsSignature(executablePath) {
  const result = spawnSync("signtool", ["remove", "/s", executablePath], {
    encoding: "utf8",
  })
  if (result.error) {
    if (result.error.code === "ENOENT") {
      return
    }
    throw new Error(
      [
        `qt-solid SEA failed to remove existing Windows signature from ${executablePath}`,
        String(result.error),
      ]
        .filter(Boolean)
        .join(": "),
    )
  }
}

function resolvePostjectBinary() {
  const binName = process.platform === "win32" ? "postject.cmd" : "postject"
  const candidate = join(repositoryRoot, "node_modules", ".bin", binName)
  return existsSync(candidate) ? candidate : null
}

function copyNodeExecutable(nodeBinary, executablePath) {
  copyFileSync(nodeBinary, executablePath)
  const mode = statSync(nodeBinary).mode & 0o777
  if (mode !== 0) {
    chmodSync(executablePath, mode)
  }
}

function prepareExecutableCopyForPostject(nodeBinary, executablePath, targetPlatform) {
  copyNodeExecutable(nodeBinary, executablePath)

  if (targetPlatform === "darwin") {
    removeMacosSignature(executablePath)
    return
  }

  if (targetPlatform === "win32") {
    removeWindowsSignature(executablePath)
  }
}

function buildSeaExecutableDirect(nodeBinary, outDir, executablePath, prep) {
  const directConfigPath = join(outDir, "sea-executable-config.json")
  writeSeaConfig(directConfigPath, {
    main: join(outDir, "sea-main.cjs"),
    mainFormat: "commonjs",
    output: executablePath,
    disableExperimentalSEAWarning: true,
    useSnapshot: false,
    useCodeCache: false,
    execArgv: ["--enable-source-maps", "--conditions=browser"],
    assets: JSON.parse(readFileSync(prep.seaConfigPath, "utf8")).assets,
  })

  const result = spawnSync(nodeBinary, ["--build-sea", directConfigPath], {
    cwd: outDir,
    encoding: "utf8",
  })
  if (result.status !== 0 || result.signal || result.error) {
    return {
      ok: false,
      reason: [
        "qt-solid SEA direct executable build failed",
        result.error ? String(result.error) : null,
        result.stderr?.trim() || null,
      ]
        .filter(Boolean)
        .join(": "),
    }
  }

  return { ok: true, reason: null }
}

function buildSeaExecutableWithPostject(nodeBinary, executablePath, prepBlobPath, targetPlatform) {
  const postjectBinary = resolvePostjectBinary()
  if (!postjectBinary) {
    return {
      ok: false,
      reason: "qt-solid SEA postject fallback unavailable: local postject binary not found",
    }
  }

  if (!binaryHasSeaFuse(nodeBinary)) {
    return {
      ok: false,
      reason: `Node binary ${nodeBinary} is missing ${seaSentinel}; postject injection cannot work`,
    }
  }

  prepareExecutableCopyForPostject(nodeBinary, executablePath, targetPlatform)
  const args = [executablePath, "NODE_SEA_BLOB", prepBlobPath, "--sentinel-fuse", seaSentinel]
  if (targetPlatform === "darwin") {
    args.push("--macho-segment-name", "NODE_SEA")
  }
  args.push("--overwrite")

  const result = spawnSync(postjectBinary, args, {
    encoding: "utf8",
  })
  if (result.status !== 0 || result.signal || result.error) {
    return {
      ok: false,
      reason: [
        "qt-solid SEA postject injection failed",
        result.error ? String(result.error) : null,
        result.stderr?.trim() || null,
      ]
        .filter(Boolean)
        .join(": "),
    }
  }

  return { ok: true, reason: null }
}

export async function buildSolidNodeSeaPrep(options) {
  const outDir = resolve(options.outDir)
  ensureDir(outDir)

  const bundleDir = join(outDir, "app")
  ensureDir(bundleDir)
  const bundlePath = join(bundleDir, "app.mjs")
  const injectedMainPath = join(outDir, "sea-main.cjs")
  const seaConfigPath = join(outDir, "sea-config.json")
  const prepBlobPath = join(outDir, "qt-solid.sea.blob")
  const prepNodeBinary = options.prepNodeBinary ?? options.nodeBinary ?? process.execPath
  const targetPlatform = options.targetPlatform ?? process.platform
  const targetArch = options.targetArch ?? process.arch

  validateNativeRuntimePackages(runtimePackageNames, targetPlatform, targetArch)

  await buildSolidNodeBundle({
    bootstrap: options.bootstrap ?? true,
    entryPath: resolve(options.entryPath),
    outfile: bundlePath,
  })

  for (const packageName of runtimePackageNames) {
    stageRuntimePackage(packageName, outDir)
  }

  const assetMap = {}
  stageSeaAsset(assetMap, "app/app.mjs", bundlePath)
  if (existsSync(`${bundlePath}.map`)) {
    stageSeaAsset(assetMap, "app/app.mjs.map", `${bundlePath}.map`)
  }
  if (options.includeDevtoolsWorker !== false && existsSync(defaultWorkerAssetPath)) {
    const stagedWorkerPath = join(bundleDir, "cdp-worker.mjs")
    copyFileSync(defaultWorkerAssetPath, stagedWorkerPath)
    stageSeaAsset(assetMap, "app/cdp-worker.mjs", stagedWorkerPath)
  }

  const runtimeRoot = join(outDir, "node_modules")
  if (existsSync(runtimeRoot)) {
    const queue = [runtimeRoot]
    while (queue.length > 0) {
      const current = queue.pop()
      if (!current) {
        continue
      }
      for (const entry of readdirSync(current)) {
        const absoluteEntry = join(current, entry)
        const stats = statSync(absoluteEntry)
        if (stats.isDirectory()) {
          queue.push(absoluteEntry)
          continue
        }
        if (!shouldCopyRuntimeFile(absoluteEntry)) {
          continue
        }
        const relativeAssetPath = absoluteEntry
          .slice(outDir.length + 1)
          .split("\\")
          .join("/")
        stageSeaAsset(assetMap, relativeAssetPath, absoluteEntry)
      }
    }
  }

  writeInjectedMain(injectedMainPath)
  writeSeaConfig(seaConfigPath, {
    main: injectedMainPath,
    output: prepBlobPath,
    disableExperimentalSEAWarning: true,
    useSnapshot: false,
    useCodeCache: false,
    execArgv: ["--enable-source-maps", "--conditions=browser"],
    assets: assetMap,
  })
  runSeaPreparation(prepNodeBinary, seaConfigPath, outDir)

  return {
    bundlePath,
    prepBlobPath,
    seaConfigPath,
  }
}

export async function buildSolidNodeSeaExecutable(options) {
  const outDir = resolve(options.outDir)
  const prepNodeBinary = options.prepNodeBinary ?? options.nodeBinary ?? process.execPath
  const targetNodeBinary = options.targetNodeBinary ?? options.nodeBinary ?? process.execPath
  const targetPlatform = options.targetPlatform ?? process.platform
  const targetArch = options.targetArch ?? process.arch
  const executableExtension = targetPlatform === "win32" ? ".exe" : ""
  const executablePath =
    options.executablePath ?? join(outDir, `qt-solid-app${executableExtension}`)

  const prep = await buildSolidNodeSeaPrep({
    ...options,
    outDir,
    prepNodeBinary,
    targetPlatform,
    targetArch,
  })

  const canUseDirectBuild =
    targetPlatform === process.platform &&
    targetArch === process.arch &&
    supportsDirectSeaBuild(prepNodeBinary)

  let attempt = canUseDirectBuild
    ? buildSeaExecutableDirect(prepNodeBinary, outDir, executablePath, prep)
    : {
        ok: false,
        reason:
          targetPlatform !== process.platform || targetArch !== process.arch
            ? `Cross-target SEA build for ${targetPlatform}/${targetArch} uses postject workflow`
            : `Node binary ${prepNodeBinary} predates --build-sea; falling back to postject workflow`,
      }

  if (!attempt.ok) {
    attempt = buildSeaExecutableWithPostject(targetNodeBinary, executablePath, prep.prepBlobPath, targetPlatform)
  }
  if (!attempt.ok) {
    return {
      ...prep,
      executablePath: null,
      capabilityError: attempt.reason,
    }
  }

  if (targetPlatform === "darwin") {
    signMacosSeaBinary(executablePath)
  }

  return {
    ...prep,
    executablePath,
    capabilityError: null,
  }
}
