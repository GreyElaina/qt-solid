/* hand-owned primitive native entry */

import { readdirSync } from "node:fs"
import { createRequire } from "node:module"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const require = createRequire(import.meta.url)
const nativeDir = dirname(fileURLToPath(import.meta.url))

function isMusl() {
  if (process.platform !== "linux") {
    return false
  }

  try {
    return require("node:fs").readFileSync("/usr/bin/ldd", "utf8").includes("musl")
  } catch {
    return false
  }
}

function resolveNativeBinaryPath() {
  const envPath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH
  if (envPath) {
    return envPath
  }

  const entries = new Set(readdirSync(nativeDir))
  const candidates = []

  if (process.platform === "darwin") {
    candidates.push("index.darwin-universal.node")
    candidates.push(`index.darwin-${process.arch}.node`)
  } else if (process.platform === "linux") {
    const abi = isMusl() ? "musl" : "gnu"
    candidates.push(`index.linux-${process.arch}-${abi}.node`)
  } else if (process.platform === "win32") {
    candidates.push(`index.win32-${process.arch}-msvc.node`)
    candidates.push(`index.win32-${process.arch}-gnu.node`)
  } else if (process.platform === "freebsd") {
    candidates.push(`index.freebsd-${process.arch}.node`)
  } else if (process.platform === "android") {
    if (process.arch === "arm64") {
      candidates.push("index.android-arm64.node")
    } else if (process.arch === "arm") {
      candidates.push("index.android-arm-eabi.node")
    }
  }

  const match = candidates.find((candidate) => entries.has(candidate))
  if (!match) {
    throw new Error(
      `No native binary for ${process.platform}/${process.arch} under ${nativeDir}`,
    )
  }

  return join(nativeDir, match)
}

const nativeBinding = require(resolveNativeBinaryPath())

export const {
  AlignItems,
  FlexDirection,
  FocusPolicy,
  JustifyContent,
  QtApp,
  QtNode,
  __qtSolidDebugCaptureWindowFrame,
  __qtSolidDebugClearHighlight,
  __qtSolidDebugClickNode,
  __qtSolidDebugCloseNode,
  __qtSolidDebugEmitAppEvent,
  __qtSolidDebugGetNodeAtPoint,
  __qtSolidDebugGetNodeBounds,
  __qtSolidDebugHighlightNode,
  __qtSolidDebugInputInsertText,
  __qtSolidDebugScheduleTimerEvent,
  __qtSolidDebugSetInspectMode,
  __qtSolidTraceClear,
  __qtSolidTraceEnterInteraction,
  __qtSolidTraceExitInteraction,
  __qtSolidTraceRecordJs,
  __qtSolidTraceSetEnabled,
  __qtSolidTraceSnapshot,
  __qtSolidWindowHostInfo,
  ping,
} = nativeBinding
