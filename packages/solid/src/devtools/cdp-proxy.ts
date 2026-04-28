import { existsSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import { Worker } from "node:worker_threads"

import { qtSolidDebugPrimitives } from "./debug-primitives.ts"
import {
  rendererInspectorStore,
  type DevtoolsEvent,
} from "./inspector-store.ts"


export interface QtSolidDevtoolsServer {
  url: string
  notifyInspectNode(nodeId: number): void
  dispose(): Promise<void>
}

let activeServer: QtSolidDevtoolsServer | null = null

interface DevtoolsEventMessage {
  type: "devtools-event"
  event: DevtoolsEvent
}

interface MetadataSnapshotMessage {
  type: "metadata-snapshot"
  metadata: Array<{ canvasNodeId: number; fragmentId: number; source: unknown; owner: unknown }>
  canvasNodeIds: number[]
}

interface InspectNodeMessage {
  type: "inspect-node"
  rendererNodeId: number
}

interface NativeRequestMessage {
  type: "native-request"
  requestId: number
  method: string
  params?: Record<string, unknown>
}

interface TreeSnapshotMessage {
  type: "tree-snapshot"
  canvasNodeId: number
  snapshot: unknown[]
}

interface NativeResponseMessage {
  type: "native-response"
  requestId: number
  result?: unknown
  error?: string
}

function postEvent(worker: Worker, event: DevtoolsEvent): void {
  const message: DevtoolsEventMessage = { type: "devtools-event", event }
  worker.postMessage(message)
}

function postTreeSnapshot(worker: Worker, canvasNodeId: number): void {
  const snapshot = qtSolidDebugPrimitives.fragmentTreeSnapshot(canvasNodeId)
  const message: TreeSnapshotMessage = { type: "tree-snapshot", canvasNodeId, snapshot }
  worker.postMessage(message)
}

function postAllTreeSnapshots(worker: Worker): void {
  for (const canvasNodeId of rendererInspectorStore.getCanvasNodeIds()) {
    postTreeSnapshot(worker, canvasNodeId)
  }
}

function postFullSnapshot(worker: Worker): void {
  const message: MetadataSnapshotMessage = {
    type: "metadata-snapshot",
    metadata: rendererInspectorStore.metadataSnapshot(),
    canvasNodeIds: [...rendererInspectorStore.getCanvasNodeIds()],
  }
  worker.postMessage(message)
  postAllTreeSnapshots(worker)
}

function resolveWorkerEntry(): string | URL {
  const localWorkerUrl = new URL("./cdp-worker.mjs", import.meta.url)
  if (existsSync(fileURLToPath(localWorkerUrl))) {
    return localWorkerUrl
  }

  const packageEntryPath = fileURLToPath(import.meta.resolve("@qt-solid/solid"))
  const packageEntryDir = dirname(packageEntryPath)
  const packageWorkerPath = join(packageEntryDir, "devtools/cdp-worker.mjs")
  if (existsSync(packageWorkerPath)) {
    return packageWorkerPath
  }

  throw new Error(`Could not resolve cdp-worker.mjs from ${fileURLToPath(localWorkerUrl)} or ${packageWorkerPath}`)
}

async function handleNativeRequest(message: NativeRequestMessage): Promise<NativeResponseMessage> {
  const params = message.params ?? {}

  try {
    switch (message.method) {
      case "highlightNode": {
        const rendererNodeId = typeof params.rendererNodeId === "number" ? params.rendererNodeId : 0
        qtSolidDebugPrimitives.highlightNode(rendererNodeId)
        return { type: "native-response", requestId: message.requestId, result: {} }
      }
      case "getNodeBounds": {
        const rendererNodeId = typeof params.rendererNodeId === "number" ? params.rendererNodeId : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.getNodeBounds(rendererNodeId),
        }
      }
      case "getNodeAtPoint": {
        const screenX = typeof params.screenX === "number" ? params.screenX : 0
        const screenY = typeof params.screenY === "number" ? params.screenY : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.getNodeAtPoint(screenX, screenY),
        }
      }
      case "setInspectMode": {
        qtSolidDebugPrimitives.setInspectMode(params.enabled === true)
        return { type: "native-response", requestId: message.requestId, result: {} }
      }
      case "clearHighlight": {
        qtSolidDebugPrimitives.clearHighlight()
        return { type: "native-response", requestId: message.requestId, result: {} }
      }
      case "highlightFragment": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        const fragmentId = typeof params.fragmentId === "number" ? params.fragmentId : null
        qtSolidDebugPrimitives.highlightFragment(canvasNodeId, fragmentId)
        return { type: "native-response", requestId: message.requestId, result: {} }
      }
      case "clearFragmentHighlight": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        qtSolidDebugPrimitives.highlightFragment(canvasNodeId, null)
        return { type: "native-response", requestId: message.requestId, result: {} }
      }
      case "getFragmentBounds": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        const fragmentId = typeof params.fragmentId === "number" ? params.fragmentId : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.getFragmentBounds(canvasNodeId, fragmentId),
        }
      }
      case "fragmentHitTest": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        const x = typeof params.x === "number" ? params.x : 0
        const y = typeof params.y === "number" ? params.y : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.fragmentHitTest(canvasNodeId, x, y),
        }
      }
      case "snapshotLayers": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.snapshotLayers(canvasNodeId),
        }
      }
      case "snapshotAnimations": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.snapshotAnimations(canvasNodeId),
        }
      }
      case "captureLayerSnapshot": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        const x = typeof params.x === "number" ? params.x : 0
        const y = typeof params.y === "number" ? params.y : 0
        const width = typeof params.width === "number" ? params.width : 0
        const height = typeof params.height === "number" ? params.height : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.captureFragmentRegion(canvasNodeId, x, y, width, height),
        }
      }
      case "captureCanvasFullSnapshot": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.captureCanvasFullSnapshot(canvasNodeId),
        }
      }
      case "captureFragmentIsolated": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        const fragmentId = typeof params.fragmentId === "number" ? params.fragmentId : 0
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.captureFragmentIsolated(canvasNodeId, fragmentId),
        }
      }
      case "captureAllFragmentsIsolated": {
        const canvasNodeId = typeof params.canvasNodeId === "number" ? params.canvasNodeId : 0
        const fragmentIds = Array.isArray(params.fragmentIds) ? params.fragmentIds.filter((v: unknown) => typeof v === "number") : []
        return {
          type: "native-response",
          requestId: message.requestId,
          result: qtSolidDebugPrimitives.captureAllFragmentsIsolated(canvasNodeId, fragmentIds),
        }
      }
      default:
        return {
          type: "native-response",
          requestId: message.requestId,
          error: `Unsupported native method ${message.method}`,
        }
    }
  } catch (error) {
    return {
      type: "native-response",
      requestId: message.requestId,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}

export async function startQtSolidDevtoolsServer(port = Number(process.env.QT_SOLID_DEVTOOLS_PORT ?? "9229")): Promise<QtSolidDevtoolsServer> {
  if (activeServer) {
    return activeServer
  }

  const worker = new Worker(resolveWorkerEntry(), {
    workerData: { port },
  })

  let resolveReady: ((url: string) => void) | null = null
  let rejectReady: ((error: Error) => void) | null = null
  const ready = new Promise<string>((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })

  const pendingTreePush = new Set<number>()
  let treePushScheduled = false

  function scheduleTreePush(canvasNodeId: number): void {
    pendingTreePush.add(canvasNodeId)
    if (!treePushScheduled) {
      treePushScheduled = true
      queueMicrotask(() => {
        treePushScheduled = false
        for (const id of pendingTreePush) {
          postTreeSnapshot(worker, id)
        }
        pendingTreePush.clear()
      })
    }
  }

  const STRUCTURAL_EVENT_TYPES: ReadonlySet<string> = new Set([
    "canvas-added", "canvas-removed",
    "node-created", "node-inserted", "node-removed", "node-destroyed",
  ])

  const unsubscribe = rendererInspectorStore.subscribe((event) => {
    postEvent(worker, event)
    if (STRUCTURAL_EVENT_TYPES.has(event.type) && "canvasNodeId" in event) {
      scheduleTreePush(event.canvasNodeId)
    }
  })

  const cleanup = () => {
    unsubscribe()
    if (activeServer?.url === resolvedUrl) {
      activeServer = null
    }
  }

  let resolvedUrl = ""

  worker.on("message", (raw: unknown) => {
    const message = raw as { type?: string; url?: string; requestId?: number; method?: string; params?: Record<string, unknown> }

    if (message.type === "ready") {
      postFullSnapshot(worker)
      resolvedUrl = String(message.url ?? "")
      resolveReady?.(resolvedUrl)
      resolveReady = null
      rejectReady = null
      return
    }

    if (message.type === "native-request" && typeof message.requestId === "number" && typeof message.method === "string") {
      void handleNativeRequest(message as NativeRequestMessage).then((response) => {
        worker.postMessage(response)
      })
    }
  })

  worker.once("error", (error) => {
    rejectReady?.(error instanceof Error ? error : new Error(String(error)))
    resolveReady = null
    rejectReady = null
    cleanup()
  })

  worker.once("exit", () => {
    cleanup()
  })

  const url = await ready

  activeServer = {
    url,
    notifyInspectNode(nodeId: number) {
      const message: InspectNodeMessage = {
        type: "inspect-node",
        rendererNodeId: nodeId,
      }
      worker.postMessage(message)
    },
    async dispose() {
      cleanup()
      await worker.terminate()
      activeServer = null
    },
  }

  console.log(`[qt-solid devtools] ${activeServer.url}`)
  return activeServer
}
