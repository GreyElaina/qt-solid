import { createHash } from "node:crypto"
import { spawnSync } from "node:child_process"
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const seaSentinel = "NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2"
const projectRoot = fileURLToPath(new URL("..", import.meta.url))

function ensureDir(path) {
  mkdirSync(path, { recursive: true })
}

export function nodeArchiveInfo(version, platform = process.platform, arch = process.arch) {
  if (platform === "darwin") {
    if (arch !== "arm64" && arch !== "x64") {
      throw new Error(`Unsupported macOS architecture for SEA node: ${arch}`)
    }
    return {
      directoryName: `node-v${version}-darwin-${arch}`,
      fileName: `node-v${version}-darwin-${arch}.tar.gz`,
      executableRelativePath: join(`node-v${version}-darwin-${arch}`, "bin", "node"),
    }
  }

  if (platform === "linux") {
    const archLabel = arch === "arm64" ? "arm64" : arch === "x64" ? "x64" : null
    if (!archLabel) {
      throw new Error(`Unsupported Linux architecture for SEA node: ${arch}`)
    }
    return {
      directoryName: `node-v${version}-linux-${archLabel}`,
      fileName: `node-v${version}-linux-${archLabel}.tar.xz`,
      executableRelativePath: join(`node-v${version}-linux-${archLabel}`, "bin", "node"),
    }
  }

  if (platform === "win32") {
    const archLabel = arch === "arm64" ? "arm64" : arch === "x64" ? "x64" : arch === "ia32" ? "x86" : null
    if (!archLabel) {
      throw new Error(`Unsupported Windows architecture for SEA node: ${arch}`)
    }
    return {
      directoryName: `node-v${version}-win-${archLabel}`,
      fileName: archLabel === "x86" ? "win-x86/node.exe" : `win-${archLabel}/node.exe`,
      executableRelativePath: join(`node-v${version}-win-${archLabel}`, "node.exe"),
    }
  }

  throw new Error(`Unsupported platform for SEA node: ${platform}`)
}

function binaryHasSeaFuse(executablePath) {
  return existsSync(executablePath) && readFileSync(executablePath).includes(Buffer.from(seaSentinel))
}

async function downloadFile(url, targetPath) {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`)
  }
  const bytes = Buffer.from(await response.arrayBuffer())
  writeFileSync(targetPath, bytes)
}

async function fetchText(url) {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`)
  }
  return response.text()
}

function verifySha256(downloadPath, shasums, fileName) {
  const expectedLine = shasums
    .split("\n")
    .map((line) => line.trim())
    .find((line) => line.endsWith(`  ${fileName}`))
  if (!expectedLine) {
    throw new Error(`Missing checksum entry for ${fileName}`)
  }

  const [expectedHash] = expectedLine.split(/\s+/, 1)
  const actualHash = createHash("sha256").update(readFileSync(downloadPath)).digest("hex")
  if (actualHash !== expectedHash) {
    throw new Error(`Checksum mismatch for ${fileName}`)
  }
}

function extractArchive(archivePath, destinationDir) {
  const result = spawnSync("tar", ["-xf", archivePath, "-C", destinationDir], {
    encoding: "utf8",
  })
  if (result.status !== 0 || result.signal || result.error) {
    throw new Error(
      [
        "Failed to extract SEA node archive",
        result.error ? String(result.error) : null,
        result.stderr?.trim() || null,
      ]
        .filter(Boolean)
        .join(": "),
    )
  }
}

async function materializeSeaNodeBinary(cacheRoot, version, platform, arch) {
  const info = nodeArchiveInfo(version, platform, arch)
  const releaseBase = `https://nodejs.org/dist/v${version}`
  const executablePath = join(cacheRoot, info.executableRelativePath)
  if (binaryHasSeaFuse(executablePath)) {
    return executablePath
  }

  rmSync(cacheRoot, { recursive: true, force: true })
  ensureDir(cacheRoot)

  const shasums = await fetchText(`${releaseBase}/SHASUMS256.txt`)
  const downloadPath = join(cacheRoot, info.fileName.split("/").at(-1))
  await downloadFile(`${releaseBase}/${info.fileName}`, downloadPath)
  verifySha256(downloadPath, shasums, info.fileName)

  if (platform === "win32") {
    const targetDir = join(cacheRoot, info.directoryName)
    ensureDir(targetDir)
    copyFileSync(downloadPath, executablePath)
  } else {
    extractArchive(downloadPath, cacheRoot)
  }

  chmodSync(executablePath, 0o755)
  if (!binaryHasSeaFuse(executablePath)) {
    throw new Error(`Official Node ${version} binary does not expose ${seaSentinel}`)
  }
  return executablePath
}

export async function ensureSeaNodeBinary(options = {}) {
  const version = options.version ?? process.version.slice(1)
  const platform = options.platform ?? process.platform
  const arch = options.arch ?? process.arch
  const cacheRoot = resolve(
    options.cacheRoot ?? join(projectRoot, ".cache", "qt-solid-sea-node", `v${version}`, `${platform}-${arch}`),
  )
  return materializeSeaNodeBinary(cacheRoot, version, platform, arch)
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const binary = await ensureSeaNodeBinary()
  console.log(binary)
}
