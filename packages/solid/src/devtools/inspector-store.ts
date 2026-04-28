import type { QtSolidOwnerMetadata } from "./owner-metadata.ts"
import type { QtSolidSourceMetadata } from "./source-metadata.ts"

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FragmentMetadata {
  source: QtSolidSourceMetadata | null
  owner: QtSolidOwnerMetadata | null
}

export type DevtoolsEvent =
  | { type: "canvas-added"; canvasNodeId: number }
  | { type: "canvas-removed"; canvasNodeId: number }
  | { type: "node-created"; canvasNodeId: number; fragmentId: number; kind: string }
  | { type: "node-inserted"; canvasNodeId: number; parentFragmentId: number | null; childFragmentId: number; anchorFragmentId: number | null }
  | { type: "node-removed"; canvasNodeId: number; parentFragmentId: number | null; childFragmentId: number }
  | { type: "node-destroyed"; canvasNodeId: number; fragmentId: number }
  | { type: "text-changed"; canvasNodeId: number; fragmentId: number; value: string }
  | { type: "prop-changed"; canvasNodeId: number; fragmentId: number; key: string }

type DevtoolsEventListener = (event: DevtoolsEvent) => void

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

class RendererInspectorStore {
  private readonly metadata = new Map<string, FragmentMetadata>()
  private readonly canvasNodeIds = new Set<number>()
  private readonly listeners = new Set<DevtoolsEventListener>()

  private metaKey(canvasNodeId: number, fragmentId: number): string {
    return `${canvasNodeId}:${fragmentId}`
  }

  // --- Canvas lifecycle ---

  addCanvas(canvasNodeId: number): void {
    if (this.canvasNodeIds.has(canvasNodeId)) {
      return
    }
    this.canvasNodeIds.add(canvasNodeId)
    this.emit({ type: "canvas-added", canvasNodeId })
  }

  removeCanvas(canvasNodeId: number): void {
    this.canvasNodeIds.delete(canvasNodeId)
    for (const key of this.metadata.keys()) {
      if (key.startsWith(`${canvasNodeId}:`)) {
        this.metadata.delete(key)
      }
    }
    this.emit({ type: "canvas-removed", canvasNodeId })
  }

  getCanvasNodeIds(): ReadonlySet<number> {
    return this.canvasNodeIds
  }

  // --- Metadata ---

  setSource(canvasNodeId: number, fragmentId: number, source: QtSolidSourceMetadata): void {
    const key = this.metaKey(canvasNodeId, fragmentId)
    let meta = this.metadata.get(key)
    if (!meta) {
      meta = { source: null, owner: null }
      this.metadata.set(key, meta)
    }
    meta.source = source
  }

  clearSource(canvasNodeId: number, fragmentId: number): void {
    const meta = this.metadata.get(this.metaKey(canvasNodeId, fragmentId))
    if (meta) meta.source = null
  }

  setOwner(canvasNodeId: number, fragmentId: number, owner: QtSolidOwnerMetadata): void {
    const key = this.metaKey(canvasNodeId, fragmentId)
    let meta = this.metadata.get(key)
    if (!meta) {
      meta = { source: null, owner: null }
      this.metadata.set(key, meta)
    }
    meta.owner = owner
  }

  clearOwner(canvasNodeId: number, fragmentId: number): void {
    const meta = this.metadata.get(this.metaKey(canvasNodeId, fragmentId))
    if (meta) meta.owner = null
  }

  getMetadata(canvasNodeId: number, fragmentId: number): FragmentMetadata | undefined {
    return this.metadata.get(this.metaKey(canvasNodeId, fragmentId))
  }

  removeNode(canvasNodeId: number, fragmentId: number): void {
    this.metadata.delete(this.metaKey(canvasNodeId, fragmentId))
  }

  // --- Snapshot (metadata only, for serialization to worker) ---

  metadataSnapshot(): Array<{
    canvasNodeId: number
    fragmentId: number
    source: QtSolidSourceMetadata | null
    owner: QtSolidOwnerMetadata | null
  }> {
    const result: Array<{
      canvasNodeId: number
      fragmentId: number
      source: QtSolidSourceMetadata | null
      owner: QtSolidOwnerMetadata | null
    }> = []
    for (const [key, meta] of this.metadata) {
      const [canvasStr, fragStr] = key.split(":")
      result.push({
        canvasNodeId: Number(canvasStr),
        fragmentId: Number(fragStr),
        source: meta.source,
        owner: meta.owner,
      })
    }
    return result
  }

  // --- Event emission ---

  emit(event: DevtoolsEvent): void {
    for (const listener of this.listeners) {
      listener(event)
    }
  }

  subscribe(listener: DevtoolsEventListener): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }
}

export const rendererInspectorStore = new RendererInspectorStore()
