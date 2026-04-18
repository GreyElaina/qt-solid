import type { QtSolidOwnerMetadata } from "./owner-metadata.ts"
import { sameQtSolidOwnerMetadata } from "./owner-metadata.ts"
import { sameQtSolidSourceMetadata, type QtSolidSourceMetadata } from "./source-metadata.ts"

export interface RendererInspectorNode {
  id: number
  kind: string
  text: string | null
  source: QtSolidSourceMetadata | null
  owner: QtSolidOwnerMetadata | null
  props: Map<string, unknown>
  listeners: Set<string>
  parentId: number | null
  childIds: number[]
}

export interface RendererInspectorSnapshotNode {
  id: number
  kind: string
  text: string | null
  source: QtSolidSourceMetadata | null
  owner: QtSolidOwnerMetadata | null
  props: Record<string, unknown>
  listeners: string[]
  parentId: number | null
  childIds: number[]
}

export interface RendererInspectorSnapshot {
  rootId: number | null
  nodes: RendererInspectorSnapshotNode[]
  revision: number
}

export type RendererInspectorMutation =
  | { type: "document-reset" }
  | { type: "node-inserted"; parentId: number; nodeId: number; previousSiblingId: number | null }
  | { type: "node-removed"; parentId: number; nodeId: number }
  | { type: "node-attribute-changed"; nodeId: number; name: string }
  | { type: "node-attribute-removed"; nodeId: number; name: string }
  | { type: "node-text-changed"; nodeId: number }

type StoreListener = (mutation: RendererInspectorMutation) => void

class RendererInspectorStore {
  private rootId: number | null = null
  private revision = 0
  private readonly nodes = new Map<number, RendererInspectorNode>()
  private readonly listeners = new Set<StoreListener>()

  reset(rootId: number): void {
    this.rootId = rootId
    this.nodes.clear()
    this.nodes.set(rootId, {
      id: rootId,
      kind: "qt-root",
      text: null,
      source: null,
      owner: null,
      props: new Map(),
      listeners: new Set(),
      parentId: null,
      childIds: [],
    })
    this.bump({ type: "document-reset" })
  }

  ensureElementNode(id: number, kind: string): void {
    const existing = this.nodes.get(id)
    if (existing) {
      if (existing.kind === kind) {
        return
      }

      existing.kind = kind
      this.bump()
      return
    }

    this.nodes.set(id, {
      id,
      kind,
      text: null,
      source: null,
      owner: null,
      props: new Map(),
      listeners: new Set(),
      parentId: null,
      childIds: [],
    })
    this.bump()
  }

  ensureTextNode(id: number, value: string): void {
    const existing = this.nodes.get(id)
    if (existing) {
      const changed = existing.kind !== "#text" || existing.text !== value
      if (!changed) {
        return
      }

      existing.kind = "#text"
      existing.text = value
      this.bump()
      return
    }

    this.nodes.set(id, {
      id,
      kind: "#text",
      text: value,
      source: null,
      owner: null,
      props: new Map(),
      listeners: new Set(),
      parentId: null,
      childIds: [],
    })
    this.bump()
  }

  replaceText(id: number, value: string): void {
    const node = this.nodes.get(id)
    if (!node || node.text === value) {
      return
    }

    node.text = value
    this.bump(this.isConnectedId(id) ? { type: "node-text-changed", nodeId: id } : undefined)
  }

  setSource(id: number, source: QtSolidSourceMetadata): void {
    const node = this.nodes.get(id)
    if (!node || sameQtSolidSourceMetadata(node.source, source)) {
      return
    }

    node.source = source
    this.bump(this.isConnectedId(id) ? { type: "node-attribute-changed", nodeId: id, name: "source" } : undefined)
  }

  clearSource(id: number): void {
    const node = this.nodes.get(id)
    if (!node?.source) {
      return
    }

    node.source = null
    this.bump(this.isConnectedId(id) ? { type: "node-attribute-removed", nodeId: id, name: "source" } : undefined)
  }

  setOwner(id: number, owner: QtSolidOwnerMetadata): void {
    const node = this.nodes.get(id)
    if (!node || sameQtSolidOwnerMetadata(node.owner, owner)) {
      return
    }

    node.owner = owner
    this.bump(this.isConnectedId(id) ? { type: "node-attribute-changed", nodeId: id, name: "owner-component" } : undefined)
  }

  clearOwner(id: number): void {
    const node = this.nodes.get(id)
    if (!node?.owner) {
      return
    }

    node.owner = null
    this.bump(this.isConnectedId(id) ? { type: "node-attribute-removed", nodeId: id, name: "owner-component" } : undefined)
  }

  setProp(id: number, key: string, value: unknown): void {
    const node = this.nodes.get(id)
    if (!node) {
      return
    }

    if (Object.is(node.props.get(key), value)) {
      return
    }

    node.props.set(key, value)
    this.bump(this.isConnectedId(id) ? { type: "node-attribute-changed", nodeId: id, name: key } : undefined)
  }

  clearProp(id: number, key: string): void {
    const node = this.nodes.get(id)
    if (!node) {
      return
    }

    if (!node.props.delete(key)) {
      return
    }

    this.bump(this.isConnectedId(id) ? { type: "node-attribute-removed", nodeId: id, name: key } : undefined)
  }

  setListener(id: number, key: string, enabled: boolean): void {
    const node = this.nodes.get(id)
    if (!node) {
      return
    }

    const hadListeners = node.listeners.size > 0
    const changed = enabled ? !node.listeners.has(key) : node.listeners.has(key)
    if (!changed) {
      return
    }

    if (enabled) {
      node.listeners.add(key)
    } else {
      node.listeners.delete(key)
    }

    if (!this.isConnectedId(id)) {
      this.bump()
      return
    }

    this.bump(
      node.listeners.size > 0 || hadListeners
        ? {
            type: node.listeners.size > 0 ? "node-attribute-changed" : "node-attribute-removed",
            nodeId: id,
            name: "listeners",
          }
        : undefined,
    )
  }

  insertChild(parentId: number, childId: number, anchorId?: number): void {
    const parent = this.nodes.get(parentId)
    const child = this.nodes.get(childId)
    if (!parent || !child) {
      return
    }

    const previousParentId = child.parentId
    const childWasConnected = this.isConnectedId(childId)
    const previousParentWasConnected = previousParentId != null ? this.isConnectedId(previousParentId) : false

    if (previousParentId != null) {
      const previousParent = this.nodes.get(previousParentId)
      if (previousParent) {
        previousParent.childIds = previousParent.childIds.filter((id) => id !== childId)
      }
    }

    parent.childIds = parent.childIds.filter((id) => id !== childId)

    if (anchorId != null) {
      const anchorIndex = parent.childIds.indexOf(anchorId)
      if (anchorIndex >= 0) {
        parent.childIds.splice(anchorIndex, 0, childId)
      } else {
        parent.childIds.push(childId)
      }
    } else {
      parent.childIds.push(childId)
    }

    child.parentId = parentId

    let emitted = false

    if (childWasConnected && previousParentId != null && previousParentWasConnected) {
      this.bump({ type: "node-removed", parentId: previousParentId, nodeId: childId })
      emitted = true
    }

    if (this.isConnectedId(parentId)) {
      const childIndex = parent.childIds.indexOf(childId)
      const previousSiblingId = childIndex > 0 ? parent.childIds[childIndex - 1] ?? null : null
      this.bump({
        type: "node-inserted",
        parentId,
        nodeId: childId,
        previousSiblingId,
      })
      emitted = true
    }

    if (!emitted) {
      this.bump()
    }
  }

  removeChild(parentId: number, childId: number): void {
    const parent = this.nodes.get(parentId)
    const child = this.nodes.get(childId)
    if (!parent || !child) {
      return
    }

    const removed = parent.childIds.includes(childId)
    if (!removed) {
      return
    }

    const parentWasConnected = this.isConnectedId(parentId)
    parent.childIds = parent.childIds.filter((id) => id !== childId)
    child.parentId = null

    this.bump(parentWasConnected ? { type: "node-removed", parentId, nodeId: childId } : undefined)
  }

  destroySubtree(id: number): void {
    const node = this.nodes.get(id)
    if (!node) {
      return
    }

    for (const childId of [...node.childIds]) {
      this.destroySubtree(childId)
    }

    if (node.parentId != null) {
      const parent = this.nodes.get(node.parentId)
      if (parent) {
        parent.childIds = parent.childIds.filter((childId) => childId !== id)
      }
    }

    this.nodes.delete(id)
    this.bump()
  }

  getNode(id: number): RendererInspectorNode | undefined {
    return this.nodes.get(id)
  }

  snapshot(): RendererInspectorSnapshot {
    return {
      rootId: this.rootId,
      revision: this.revision,
      nodes: [...this.nodes.values()]
        .map((node) => ({
          id: node.id,
          kind: node.kind,
          text: node.text,
          source: node.source,
          owner: node.owner,
          props: Object.fromEntries(node.props.entries()),
          listeners: [...node.listeners].sort(),
          parentId: node.parentId,
          childIds: [...node.childIds],
        }))
        .sort((left, right) => left.id - right.id),
    }
  }

  subscribe(listener: StoreListener): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  private isConnectedId(id: number): boolean {
    if (this.rootId == null) {
      return false
    }

    let currentId: number | null = id
    while (currentId != null) {
      if (currentId === this.rootId) {
        return true
      }

      const current = this.nodes.get(currentId)
      currentId = current?.parentId ?? null
    }

    return false
  }

  private bump(mutation?: RendererInspectorMutation): void {
    this.revision += 1
    if (!mutation) {
      return
    }

    for (const listener of this.listeners) {
      listener(mutation)
    }
  }
}

export const rendererInspectorStore = new RendererInspectorStore()
