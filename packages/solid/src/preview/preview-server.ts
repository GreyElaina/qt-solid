import { fork, type ChildProcess } from "node:child_process"
import { writeFile, mkdtemp, mkdir, rm } from "node:fs/promises"
import { join, dirname } from "node:path"
import { watch, type FSWatcher } from "node:fs"
import { createServer, type Server } from "node:http"

import { rolldown } from "rolldown"
import { WebSocketServer, type WebSocket } from "ws"

import { generatePreviewEntry, type PreviewEntryOptions } from "./virtual-entry.ts"
import { createQtSolidRolldownPlugin } from "../build/rolldown.ts"
import { extractPropsSchema } from "./extract-props.ts"
import {
  codeToFigmaTokens,
  figmaToCodeTheme,
  type FigmaTokenExport,
} from "./token-sync.ts"
import type { HostToPreviewMessage, PreviewToHostMessage, PreviewPropsSchema } from "./protocol.ts"

export interface PreviewServerOptions {
  filePath: string
  exportName: string
  title?: string
  width?: number
  height?: number
  /** Enable file watching for hot-reload. Default: true */
  watch?: boolean
  /** Enable devtools. Default: true */
  devtools?: boolean
  /** Enable IPC props control. Default: true */
  propsControl?: boolean
  /** WebSocket port for Figma plugin bridge. Default: 9230 */
  wsPort?: number
  /** Theme file path for bidirectional token sync. */
  themeFile?: string
}

export interface PreviewServer {
  /** Rebuild and restart the preview */
  rebuild(): Promise<void>
  /** Send a message to the preview child */
  send(msg: HostToPreviewMessage): void
  /** Register a handler for messages from the child */
  onMessage(handler: (msg: PreviewToHostMessage) => void): void
  /** Current props schema (null until child reports it) */
  schema: PreviewPropsSchema | null
  /** WebSocket server port (null if not started) */
  wsPort: number | null
  /** Stop the preview server and clean up */
  dispose(): Promise<void>
}

export async function createPreviewServer(
  options: PreviewServerOptions,
): Promise<PreviewServer> {
  const {
    filePath,
    exportName,
    title,
    width,
    height,
    watch: enableWatch = true,
    devtools = true,
    propsControl = true,
    wsPort = 9230,
    themeFile,
  } = options

  const cacheDir = join(process.cwd(), ".cache", "qt-solid-preview")
  await mkdir(cacheDir, { recursive: true })
  const tmpDir = await mkdtemp(join(cacheDir, "session-"))
  const entryPath = join(tmpDir, "entry.tsx")
  const outputPath = join(tmpDir, "output.mjs")

  let child: ChildProcess | null = null
  let watcher: FSWatcher | null = null
  let themeWatcher: FSWatcher | null = null
  let debounceTimer: ReturnType<typeof setTimeout> | null = null
  let currentSchema: PreviewPropsSchema | null = null
  const messageHandlers = new Set<(msg: PreviewToHostMessage) => void>()

  // WebSocket server for Figma plugin bridge
  const clients = new Set<WebSocket>()
  let httpServer: Server | null = null
  let wss: WebSocketServer | null = null
  let actualWsPort: number | null = null

  const entryOptions: PreviewEntryOptions = {
    filePath,
    exportName,
    title,
    width,
    height,
    propsControl,
  }

  // ---------------------------------------------------------------------------
  // Build + child management (unchanged logic)
  // ---------------------------------------------------------------------------

  async function build(): Promise<boolean> {
    if (propsControl) {
      const extracted = extractPropsSchema(filePath, exportName)
      entryOptions.propsSchema = extracted.props
      entryOptions.variantAxes = extracted.variantAxes
    }

    const source = generatePreviewEntry(entryOptions)
    await writeFile(entryPath, source, "utf-8")

    try {
      const bundle = await rolldown({
        input: entryPath,
        platform: "node",
        plugins: [createQtSolidRolldownPlugin()],
        resolve: { conditionNames: ["browser"] },
      })

      await bundle.write({
        file: outputPath,
        format: "esm",
        sourcemap: true,
        codeSplitting: false,
      })

      await bundle.close()
      return true
    } catch (err) {
      console.error("[qt-solid preview] Build failed:", err)
      return false
    }
  }

  function spawnChild(): ChildProcess {
    const env: Record<string, string> = { ...process.env } as Record<string, string>
    if (devtools) {
      env.QT_SOLID_DEVTOOLS = "1"
    }

    const projectNodeModules = join(process.cwd(), "node_modules")
    env.NODE_PATH = env.NODE_PATH
      ? `${projectNodeModules}:${env.NODE_PATH}`
      : projectNodeModules

    const proc = fork(outputPath, [], {
      execArgv: ["--enable-source-maps", "--conditions=browser"],
      env,
      stdio: "inherit",
      cwd: process.cwd(),
    })

    if (propsControl) {
      proc.on("message", (msg: PreviewToHostMessage) => {
        if (msg.type === "schema") {
          currentSchema = msg.schema
        }
        for (const handler of messageHandlers) {
          handler(msg)
        }
        // Forward schema to Figma clients
        broadcast({ type: "preview-schema", schema: msg })
      })
    }

    return proc
  }

  function killChild(): Promise<void> {
    if (!child) return Promise.resolve()

    return new Promise<void>((resolve) => {
      child!.once("exit", () => {
        child = null
        resolve()
      })
      child!.kill("SIGTERM")
    })
  }

  async function rebuild(): Promise<void> {
    const ok = await build()
    if (!ok) return

    await killChild()
    child = spawnChild()
  }

  // ---------------------------------------------------------------------------
  // WebSocket server — Figma plugin bridge
  // ---------------------------------------------------------------------------

  function broadcast(msg: Record<string, unknown>): void {
    const data = JSON.stringify(msg)
    for (const client of clients) {
      if (client.readyState === 1) {
        client.send(data)
      }
    }
  }

  function handleFigmaMessage(raw: string): void {
    let msg: Record<string, unknown>
    try {
      msg = JSON.parse(raw)
    } catch {
      return
    }

    const type = msg.type as string

    switch (type) {
      case "figma-tokens": {
        // Figma → Code: write theme file
        if (!themeFile) break
        const tokens = msg.tokens as FigmaTokenExport[]
        figmaToCodeTheme(tokens, themeFile).catch((err) => {
          console.error("[qt-solid preview] Token write failed:", err)
        })
        console.log(`[qt-solid preview] Wrote ${tokens.length} tokens to ${themeFile}`)
        break
      }

      case "figma-selection": {
        // Forward selection info to preview child if needed
        break
      }

      case "figma-change": {
        // Document changes from Figma — could trigger selective rebuild
        break
      }

      case "request-tokens": {
        // Figma plugin requests current tokens from code
        pushTokensToClients()
        break
      }

      default:
        break
    }
  }

  function pushTokensToClients(): void {
    if (!themeFile) return
    try {
      const result = codeToFigmaTokens(themeFile)
      broadcast({ type: "code-tokens", ...result })
    } catch (err) {
      console.error("[qt-solid preview] Token read failed:", err)
    }
  }

  async function startWebSocketServer(): Promise<void> {
    httpServer = createServer()
    wss = new WebSocketServer({ server: httpServer })

    wss.on("connection", (ws) => {
      clients.add(ws)
      console.log(`[qt-solid preview] Figma plugin connected (${clients.size} client(s))`)

      // Send current state on connect
      if (currentSchema) {
        ws.send(JSON.stringify({ type: "preview-schema", schema: currentSchema }))
      }
      pushTokensToClients()

      ws.on("message", (data) => {
        handleFigmaMessage(data.toString())
      })

      ws.on("close", () => {
        clients.delete(ws)
        console.log(`[qt-solid preview] Figma plugin disconnected (${clients.size} client(s))`)
      })
    })

    await new Promise<void>((resolve, reject) => {
      httpServer!.listen(wsPort, () => {
        actualWsPort = wsPort
        resolve()
      })
      httpServer!.on("error", reject)
    })
  }

  // ---------------------------------------------------------------------------
  // Theme file watcher — Code → Figma auto-push
  // ---------------------------------------------------------------------------

  function startThemeWatcher(): void {
    if (!themeFile) return

    themeWatcher = watch(themeFile, () => {
      // Debounce and push updated tokens to all Figma clients
      pushTokensToClients()
    })
  }

  // ---------------------------------------------------------------------------
  // Init
  // ---------------------------------------------------------------------------

  await rebuild()
  await startWebSocketServer()
  startThemeWatcher()

  // Source file watcher (hot-reload)
  if (enableWatch) {
    const sourceDir = dirname(filePath)
    watcher = watch(sourceDir, { recursive: true }, (_event, filename) => {
      if (debounceTimer) clearTimeout(debounceTimer)
      debounceTimer = setTimeout(() => {
        rebuild().catch((err) => {
          console.error("[qt-solid preview] Rebuild error:", err)
        })
      }, 50)
    })
  }

  console.log(`[qt-solid preview] WebSocket bridge on ws://127.0.0.1:${wsPort}`)

  // ---------------------------------------------------------------------------
  // Public API
  // ---------------------------------------------------------------------------

  async function dispose(): Promise<void> {
    if (debounceTimer) clearTimeout(debounceTimer)
    if (watcher) { watcher.close(); watcher = null }
    if (themeWatcher) { themeWatcher.close(); themeWatcher = null }
    for (const client of clients) client.close()
    clients.clear()
    if (wss) { wss.close(); wss = null }
    if (httpServer) { httpServer.close(); httpServer = null }
    await killChild()
    await rm(tmpDir, { recursive: true, force: true }).catch(() => {})
  }

  return {
    rebuild,
    send(msg: HostToPreviewMessage) {
      child?.send(msg)
    },
    onMessage(handler: (msg: PreviewToHostMessage) => void) {
      messageHandlers.add(handler)
    },
    get schema() { return currentSchema },
    get wsPort() { return actualWsPort },
    dispose,
  }
}
