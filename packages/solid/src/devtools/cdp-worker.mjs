import { createServer } from "node:http"
import inspector from "node:inspector"
import { createRequire } from "node:module"
import { dirname, isAbsolute, join, resolve as resolvePath } from "node:path"
import { pathToFileURL } from "node:url"
import { parentPort, workerData } from "node:worker_threads"
import { deflateSync } from "node:zlib"

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DOCUMENT_NODE_ID = 1
const DOM_NODE_OFFSET = 1_000
const WINDOW_FRAGMENT_ID = -1
const SYNTHETIC_TARGET_ID = "qt-solid-renderer"
const SYNTHETIC_OBJECT_PREFIX = "qt-solid-frag:"
const KNOWN_DOMAINS = [
  { name: "DOM", version: "1.3" },
  { name: "CSS", version: "1.3" },
  { name: "Overlay", version: "1.3" },
  { name: "LayerTree", version: "1.3" },
  { name: "Animation", version: "1.3" },
]

const TRANSPARENT_1X1_PNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII="

// ---------------------------------------------------------------------------
// Inline PNG encoder (worker can't import .ts modules)
// ---------------------------------------------------------------------------

const _crcTable = new Uint32Array(256)
for (let n = 0; n < 256; n++) {
  let c = n
  for (let k = 0; k < 8; k++) {
    c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1)
  }
  _crcTable[n] = c
}

function _crc32(data) {
  let crc = 0xFFFFFFFF
  for (let i = 0; i < data.length; i++) {
    crc = _crcTable[(crc ^ data[i]) & 0xFF] ^ (crc >>> 8)
  }
  return (crc ^ 0xFFFFFFFF) >>> 0
}

function _pngChunk(type, data) {
  const buf = Buffer.alloc(4 + 4 + data.length + 4)
  buf.writeUInt32BE(data.length, 0)
  buf.write(type, 4, 4, "ascii")
  data.copy(buf, 8)
  buf.writeUInt32BE(_crc32(buf.subarray(4, 8 + data.length)), 8 + data.length)
  return buf
}

function encodeRgbaPng(rgba, width, height) {
  const rowBytes = 1 + width * 4
  const raw = Buffer.alloc(rowBytes * height)
  for (let y = 0; y < height; y++) {
    const dstOffset = y * rowBytes
    raw[dstOffset] = 0
    const srcOffset = y * width * 4
    for (let i = 0; i < width * 4; i++) {
      raw[dstOffset + 1 + i] = rgba[srcOffset + i]
    }
  }

  const compressed = deflateSync(raw)
  const chunks = []
  chunks.push(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8
  ihdr[9] = 6
  chunks.push(_pngChunk("IHDR", ihdr))
  chunks.push(_pngChunk("IDAT", compressed))
  chunks.push(_pngChunk("IEND", Buffer.alloc(0)))
  return `data:image/png;base64,${Buffer.concat(chunks).toString("base64")}`
}

function cropAndEncode(canvasSnapshot, x, y, width, height) {
  const { widthPx, heightPx, stride, scaleFactor, bytes, format } = canvasSnapshot
  if (!bytes || bytes.length === 0 || widthPx === 0 || heightPx === 0) {
    return null
  }

  const scale = scaleFactor || 1
  const px = Math.round(x * scale)
  const py = Math.round(y * scale)
  const pw = Math.min(Math.round(width * scale), widthPx - px)
  const ph = Math.min(Math.round(height * scale), heightPx - py)
  if (pw <= 0 || ph <= 0) {
    return null
  }

  const isArgb = format === "argb32-premultiplied"
  const rgba = Buffer.alloc(pw * ph * 4)

  for (let row = 0; row < ph; row++) {
    const srcY = py + row
    if (srcY >= heightPx) break

    for (let col = 0; col < pw; col++) {
      const srcX = px + col
      if (srcX >= widthPx) break

      const srcOff = srcY * stride + srcX * 4
      const dstOff = (row * pw + col) * 4

      if (srcOff + 4 > bytes.length) {
        continue
      }

      let r, g, b, a
      if (isArgb) {
        a = bytes[srcOff]; r = bytes[srcOff + 1]; g = bytes[srcOff + 2]; b = bytes[srcOff + 3]
      } else {
        r = bytes[srcOff]; g = bytes[srcOff + 1]; b = bytes[srcOff + 2]; a = bytes[srcOff + 3]
      }

      // Un-premultiply
      if (a === 0) {
        rgba[dstOff] = 0; rgba[dstOff + 1] = 0; rgba[dstOff + 2] = 0; rgba[dstOff + 3] = 0
      } else if (a === 255) {
        rgba[dstOff] = r; rgba[dstOff + 1] = g; rgba[dstOff + 2] = b; rgba[dstOff + 3] = 255
      } else {
        const inv = 255 / a
        rgba[dstOff] = Math.min(255, Math.round(r * inv))
        rgba[dstOff + 1] = Math.min(255, Math.round(g * inv))
        rgba[dstOff + 2] = Math.min(255, Math.round(b * inv))
        rgba[dstOff + 3] = a
      }
    }
  }

  return encodeRgbaPng(rgba, pw, ph)
}

const require = createRequire(import.meta.url)
const wsPackageJsonPath = require.resolve("ws/package.json")
const { WebSocketServer } = require(join(dirname(wsPackageJsonPath), "index.js"))

// ---------------------------------------------------------------------------
// Infra helpers (unchanged)
// ---------------------------------------------------------------------------

function targetIdFor() {
  return SYNTHETIC_TARGET_ID
}

function websocketPath(port) {
  return `ws://127.0.0.1:${port}/devtools/page/${targetIdFor()}`
}

function devtoolsFrontendUrlForWebSocketUrl(webSocketDebuggerUrl) {
  const parsed = new URL(webSocketDebuggerUrl)
  const wsTarget = `${parsed.host}${parsed.pathname}${parsed.search}`
  return `devtools://devtools/bundled/inspector.html?ws=${encodeURIComponent(wsTarget)}`
}

function targetDescriptor(port) {
  const wsUrl = websocketPath(port)
  const targetId = targetIdFor()
  return {
    id: targetId,
    type: "page",
    title: `${process.title || "qt-solid-spike"} (renderer)`,
    description: "Qt Solid renderer target",
    url: "qt-solid://app",
    webSocketDebuggerUrl: wsUrl,
    devtoolsFrontendUrl: devtoolsFrontendUrlForWebSocketUrl(wsUrl),
  }
}

function sendJson(response, payload) {
  const body = JSON.stringify(payload)
  response.writeHead(200, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
    "cache-control": "no-store",
  })
  response.end(body)
}

function postInspector(session, method, params) {
  return new Promise((resolve, reject) => {
    session.post(method, params ?? {}, (error, result) => {
      if (error) {
        reject(error)
        return
      }

      resolve(result ?? {})
    })
  })
}

function withTimeout(promise, timeoutMs, message) {
  if (timeoutMs <= 0) {
    return promise
  }

  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(message))
    }, timeoutMs)

    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error) => {
        clearTimeout(timer)
        reject(error)
      },
    )
  })
}

// ---------------------------------------------------------------------------
// String / value helpers
// ---------------------------------------------------------------------------

function kebabCase(value) {
  return value.replace(/[A-Z]/g, (match) => `-${match.toLowerCase()}`)
}

function stringifyValue(value) {
  if (typeof value === "string") {
    return value
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value)
  }

  if (value == null) {
    return ""
  }

  return JSON.stringify(value)
}

// ---------------------------------------------------------------------------
// Source metadata helpers
// ---------------------------------------------------------------------------

function isOptionalString(value) {
  return value == null || typeof value === "string"
}

function isSourceMetadata(value) {
  if (typeof value !== "object" || value == null) {
    return false
  }

  return typeof value.fileName === "string"
    && typeof value.lineNumber === "number"
    && Number.isInteger(value.lineNumber)
    && typeof value.columnNumber === "number"
    && Number.isInteger(value.columnNumber)
    && isOptionalString(value.fileUrl)
    && isOptionalString(value.projectRootUrl)
}

function formatSourceMetadata(source) {
  return `${source.fileName}:${source.lineNumber}:${source.columnNumber}`
}

function sourceMetadataUrl(source) {
  if (source.fileUrl) {
    return source.fileUrl
  }

  const fileName = isAbsolute(source.fileName) ? source.fileName : resolvePath(process.cwd(), source.fileName)
  return pathToFileURL(fileName).href
}

function sourceMetadataFrameKind(source) {
  const url = sourceMetadataUrl(source)
  if (url.includes("/node_modules/")) {
    return "library"
  }

  if (!source.projectRootUrl) {
    return "user"
  }

  return url.startsWith(source.projectRootUrl) ? "user" : "library"
}

function serializeSourceLocation(source) {
  return {
    source: formatSourceMetadata(source),
    sourceFileName: source.fileName,
    sourceLineNumber: source.lineNumber,
    sourceColumnNumber: source.columnNumber,
    sourceUrl: sourceMetadataUrl(source),
    frameKind: sourceMetadataFrameKind(source),
  }
}

function sameSourceMetadata(left, right) {
  if (left === right) {
    return true
  }

  if (!left || !right) {
    return false
  }

  return left.fileName === right.fileName
    && left.lineNumber === right.lineNumber
    && left.columnNumber === right.columnNumber
    && left.fileUrl === right.fileUrl
    && left.projectRootUrl === right.projectRootUrl
}

// ---------------------------------------------------------------------------
// Owner metadata helpers
// ---------------------------------------------------------------------------

function ownerComponentName(owner) {
  return owner?.ownerStack?.[owner.ownerStack.length - 1]?.componentName ?? ""
}

function formatOwnerPath(owner) {
  return owner.ownerStack.map((frame) => frame.componentName).join(" > ")
}

function serializeOwnerStack(owner) {
  return owner.ownerStack.map((frame) => ({
    componentName: frame.componentName,
    ...(frame.source ? serializeSourceLocation(frame.source) : {
      source: "",
      sourceFileName: "",
      sourceLineNumber: 0,
      sourceColumnNumber: 0,
      sourceUrl: "",
      frameKind: "library",
    }),
  }))
}

// ---------------------------------------------------------------------------
// Creation frame helpers
// ---------------------------------------------------------------------------

function creationFrame(functionName, source, frameRole) {
  return {
    functionName,
    scriptId: "",
    url: sourceMetadataUrl(source),
    lineNumber: Math.max(0, source.lineNumber - 1),
    columnNumber: Math.max(0, source.columnNumber - 1),
    ...serializeSourceLocation(source),
    frameRole,
  }
}

function nodeFrameName(kind) {
  return kind === "#text" ? "text" : kind
}

function creationFrames(owner, source, kind) {
  const callFrames = []
  const leafOwnerFrame = owner?.ownerStack?.[owner.ownerStack.length - 1] ?? null

  if (source && !sameSourceMetadata(source, leafOwnerFrame?.source ?? null)) {
    callFrames.push(creationFrame(leafOwnerFrame?.componentName ?? nodeFrameName(kind), source, "node"))
  }

  if (owner) {
    for (let index = owner.ownerStack.length - 1; index >= 0; index -= 1) {
      const frame = owner.ownerStack[index]
      if (!frame?.source) {
        continue
      }

      callFrames.push(creationFrame(frame.componentName, frame.source, "owner"))
    }
  }

  return callFrames
}

function creationStackTrace(owner, source, kind) {
  const callFrames = creationFrames(owner, source, kind)
  if (callFrames.length === 0) {
    return null
  }

  return {
    description: "Qt Solid node creation",
    callFrames: callFrames.map((frame) => ({
      functionName: frame.functionName,
      scriptId: frame.scriptId,
      url: frame.url,
      lineNumber: frame.lineNumber,
      columnNumber: frame.columnNumber,
    })),
  }
}

// ---------------------------------------------------------------------------
// CSS / style helpers — now operate on native snapshot nodes
// ---------------------------------------------------------------------------

function styleEntriesForSnapshotNode(snap) {
  const entries = []

  const defaultDisplay = snap.tag === "#text" || snap.tag === "Text" ? "inline" : "flex"
  entries.push({ name: "display", value: defaultDisplay })

  if (typeof snap.x === "number") {
    entries.push({ name: "x", value: `${snap.x}px` })
  }
  if (typeof snap.y === "number") {
    entries.push({ name: "y", value: `${snap.y}px` })
  }
  if (typeof snap.width === "number" && snap.width > 0) {
    entries.push({ name: "width", value: `${snap.width}px` })
  }
  if (typeof snap.height === "number" && snap.height > 0) {
    entries.push({ name: "height", value: `${snap.height}px` })
  }

  if (typeof snap.opacity === "number" && snap.opacity !== 1) {
    entries.push({ name: "opacity", value: String(snap.opacity) })
  }
  if (snap.clip === true) {
    entries.push({ name: "overflow", value: "hidden" })
  }
  if (snap.visible === false) {
    entries.push({ name: "visibility", value: "hidden" })
  }

  const layoutKeys = new Set([
    "width", "height", "minWidth", "minHeight",
    "flexGrow", "flexShrink", "direction", "alignItems",
    "justifyContent", "gap", "padding", "visible",
  ])

  for (const [key, value] of Object.entries(snap.props ?? {})) {
    if (!layoutKeys.has(key) || value == null) {
      continue
    }

    entries.push({ name: kebabCase(key), value: stringifyValue(value) })
  }

  return entries
}

function cssStyle(entries) {
  return {
    cssProperties: entries.map((entry) => ({
      name: entry.name,
      value: entry.value,
    })),
    shorthandEntries: [],
    cssText: entries.map((entry) => `${entry.name}: ${entry.value};`).join(" "),
  }
}

function boxQuad(left, top, width, height) {
  const right = left + width
  const bottom = top + height
  return [left, top, right, top, right, bottom, left, bottom]
}

// ---------------------------------------------------------------------------
// Attribute helpers for snapshot nodes
// ---------------------------------------------------------------------------

const HIDDEN_INLINE_ATTRIBUTE_NAMES = new Set(["kind", "source", "owner-component", "owner-path", "listeners"])

function isVisibleInlineAttributeName(name) {
  return !HIDDEN_INLINE_ATTRIBUTE_NAMES.has(name)
}

function attributesForSnapshotNode(snap) {
  const attributes = []

  for (const [key, value] of Object.entries(snap.props ?? {})) {
    if (!isVisibleInlineAttributeName(key)) {
      continue
    }

    attributes.push(key, stringifyValue(value))
  }

  return attributes
}

// ---------------------------------------------------------------------------
// Remote object helpers
// ---------------------------------------------------------------------------

function buildFragmentRemoteObject(canvasNodeId, fragmentId, tag) {
  return {
    type: "object",
    subtype: "node",
    className: tag,
    description: `<${tag}>`,
    objectId: `${SYNTHETIC_OBJECT_PREFIX}${canvasNodeId}:${fragmentId}`,
  }
}

function parseSyntheticObjectId(objectId) {
  if (!objectId?.startsWith(SYNTHETIC_OBJECT_PREFIX)) {
    return null
  }

  const rest = objectId.slice(SYNTHETIC_OBJECT_PREFIX.length)

  // "window:canvasId" format
  if (rest.startsWith("window:")) {
    return { type: "window", canvasNodeId: Number(rest.slice(7)), fragmentId: WINDOW_FRAGMENT_ID }
  }

  // "canvasId:fragId" format
  const sep = rest.indexOf(":")
  if (sep < 0) {
    return null
  }

  const canvasNodeId = Number(rest.slice(0, sep))
  const fragmentId = Number(rest.slice(sep + 1))
  if (!Number.isFinite(canvasNodeId) || !Number.isFinite(fragmentId)) {
    return null
  }

  return { type: "fragment", canvasNodeId, fragmentId }
}

// ---------------------------------------------------------------------------
// Node properties for Runtime.getProperties
// ---------------------------------------------------------------------------

function nodeProperties(snap, meta) {
  const source = meta?.source ?? null
  const owner = meta?.owner ?? null
  const sourceLocation = source ? serializeSourceLocation(source) : null
  const nodeCreationFrames = creationFrames(owner, source, snap.tag)

  return [
    {
      name: "kind",
      value: { type: "string", value: snap.tag },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "source",
      value: { type: "string", value: source ? formatSourceMetadata(source) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceFileName",
      value: { type: "string", value: source?.fileName ?? "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceLineNumber",
      value: { type: "number", value: source?.lineNumber ?? 0 },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceColumnNumber",
      value: { type: "number", value: source?.columnNumber ?? 0 },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceUrl",
      value: { type: "string", value: typeof sourceLocation?.sourceUrl === "string" ? sourceLocation.sourceUrl : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceFrameKind",
      value: { type: "string", value: typeof sourceLocation?.frameKind === "string" ? sourceLocation.frameKind : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceLocation",
      value: {
        type: "object",
        subtype: sourceLocation ? undefined : "null",
        value: sourceLocation,
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "ownerComponent",
      value: { type: "string", value: owner ? ownerComponentName(owner) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "ownerPath",
      value: { type: "string", value: owner ? formatOwnerPath(owner) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "ownerStack",
      value: {
        type: "object",
        subtype: "array",
        value: owner ? serializeOwnerStack(owner) : [],
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "creationLocation",
      value: {
        type: "object",
        subtype: nodeCreationFrames[0] ? undefined : "null",
        value: nodeCreationFrames[0] ?? null,
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "creationFrames",
      value: {
        type: "object",
        subtype: "array",
        value: nodeCreationFrames,
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "props",
      value: {
        type: "object",
        subtype: "map",
        value: { ...(snap.props ?? {}) },
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "childIds",
      value: {
        type: "object",
        subtype: "array",
        value: [...(snap.childIds ?? [])],
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
  ]
}

// ---------------------------------------------------------------------------
// NodeIdAllocator — maps (canvasNodeId, fragmentId) → stable domNodeId
// ---------------------------------------------------------------------------

class NodeIdAllocator {
  constructor() {
    this.nextId = DOM_NODE_OFFSET
    this.forwardMap = new Map()   // "canvasId:fragId" → domNodeId
    this.reverseMap = new Map()   // domNodeId → { canvasNodeId, fragmentId }
  }

  _key(canvasNodeId, fragmentId) {
    return `${canvasNodeId}:${fragmentId}`
  }

  resolve(canvasNodeId, fragmentId) {
    const key = this._key(canvasNodeId, fragmentId)
    let domNodeId = this.forwardMap.get(key)
    if (domNodeId != null) {
      return domNodeId
    }

    domNodeId = this.nextId++
    this.forwardMap.set(key, domNodeId)
    this.reverseMap.set(domNodeId, { canvasNodeId, fragmentId })
    return domNodeId
  }

  lookup(domNodeId) {
    return this.reverseMap.get(domNodeId) ?? null
  }
}

// ---------------------------------------------------------------------------
// MetadataStore — receives metadata snapshots from main thread
// ---------------------------------------------------------------------------

class MetadataStore {
  constructor() {
    this.metadata = new Map()   // "canvasId:fragId" → { source, owner }
    this.canvasNodeIds = new Set()
  }

  _key(canvasNodeId, fragmentId) {
    return `${canvasNodeId}:${fragmentId}`
  }

  replaceSnapshot(metadataEntries, canvasNodeIds) {
    this.metadata.clear()
    this.canvasNodeIds = new Set(canvasNodeIds)

    for (const entry of metadataEntries) {
      const key = this._key(entry.canvasNodeId, entry.fragmentId)
      this.metadata.set(key, { source: entry.source ?? null, owner: entry.owner ?? null })
    }
  }

  handleDevtoolsEvent(event) {
    switch (event.type) {
      case "canvas-added":
        this.canvasNodeIds.add(event.canvasNodeId)
        break
      case "canvas-removed":
        this.canvasNodeIds.delete(event.canvasNodeId)
        // Clean metadata for this canvas
        for (const key of this.metadata.keys()) {
          if (key.startsWith(`${event.canvasNodeId}:`)) {
            this.metadata.delete(key)
          }
        }
        break
    }
  }

  getMetadata(canvasNodeId, fragmentId) {
    return this.metadata.get(this._key(canvasNodeId, fragmentId)) ?? null
  }

  setSource(canvasNodeId, fragmentId, source) {
    const key = this._key(canvasNodeId, fragmentId)
    let meta = this.metadata.get(key)
    if (!meta) {
      meta = { source: null, owner: null }
      this.metadata.set(key, meta)
    }
    meta.source = source
  }

  setOwner(canvasNodeId, fragmentId, owner) {
    const key = this._key(canvasNodeId, fragmentId)
    let meta = this.metadata.get(key)
    if (!meta) {
      meta = { source: null, owner: null }
      this.metadata.set(key, meta)
    }
    meta.owner = owner
  }

  removeNode(canvasNodeId, fragmentId) {
    this.metadata.delete(this._key(canvasNodeId, fragmentId))
  }

  getCanvasNodeIds() {
    return this.canvasNodeIds
  }
}

// ---------------------------------------------------------------------------
// FragmentTreeCache — lazy native snapshot cache per canvas
// ---------------------------------------------------------------------------

class FragmentTreeCache {
  constructor() {
    this.cache = new Map()       // canvasNodeId → Array<snapshot entries>
  }

  setSnapshot(canvasNodeId, snapshot) {
    if (Array.isArray(snapshot)) {
      this.cache.set(canvasNodeId, snapshot)
    }
  }

  invalidate(canvasNodeId) {
    this.cache.delete(canvasNodeId)
  }

  getSnapshot(canvasNodeId) {
    return this.cache.get(canvasNodeId) ?? []
  }

  getNode(canvasNodeId, fragmentId) {
    const snapshot = this.cache.get(canvasNodeId)
    if (!snapshot) return null
    return snapshot.find((n) => n.id === fragmentId) ?? null
  }

  getRootChildIds(canvasNodeId) {
    const snapshot = this.cache.get(canvasNodeId)
    if (!snapshot) return []
    return snapshot.filter((n) => n.parentId == null || n.parentId === undefined).map((n) => n.id)
  }
}

// ---------------------------------------------------------------------------
// NativeBridge
// ---------------------------------------------------------------------------

class NativeBridge {
  constructor() {
    this.nextRequestId = 1
    this.pending = new Map()
  }

  receive(message) {
    if (message?.type !== "native-response") {
      return
    }

    const pending = this.pending.get(message.requestId)
    if (!pending) {
      return
    }

    this.pending.delete(message.requestId)
    if (message.error) {
      pending.reject(new Error(message.error))
      return
    }

    pending.resolve(message.result)
  }

  async request(method, params = {}, timeoutMs = 250) {
    const requestId = this.nextRequestId++
    parentPort?.postMessage({
      type: "native-request",
      requestId,
      method,
      params,
    })

    return await withTimeout(
      new Promise((resolve, reject) => {
        this.pending.set(requestId, { resolve, reject })
      }),
      timeoutMs,
      `Timed out waiting for native ${method}`,
    ).finally(() => {
      this.pending.delete(requestId)
    })
  }

  async safeRequest(method, params = {}, fallback = {}, timeoutMs = 150) {
    try {
      return await this.request(method, params, timeoutMs)
    } catch {
      return fallback
    }
  }
}

// ---------------------------------------------------------------------------
// SyntheticBackend — builds CDP DOM from native snapshots + metadata
// ---------------------------------------------------------------------------

class SyntheticBackend {
  constructor(allocator, metadataStore, treeCache, nativeBridge, sendPayload, isPaused) {
    this.allocator = allocator
    this.metadataStore = metadataStore
    this.treeCache = treeCache
    this.nativeBridge = nativeBridge
    this.sendPayload = sendPayload
    this.isPaused = isPaused
    this.domEnabled = false
    this.layerTreeEnabled = false
    this.knownDomNodeIds = new Set([DOCUMENT_NODE_ID])
    this.documentUpdatedTimer = null
    this.layerTreeTimer = null
    this.layerSnapshots = new Map()
    this.nextSnapshotId = 1
    this.lastLayerTreeSignature = null
    this._layerTreeRunning = false
    this._layerTreePendingRefresh = false
  }

  dispose() {
    if (this.documentUpdatedTimer != null) {
      clearTimeout(this.documentUpdatedTimer)
      this.documentUpdatedTimer = null
    }
    if (this.layerTreeTimer != null) {
      clearTimeout(this.layerTreeTimer)
      this.layerTreeTimer = null
    }
  }

  notifyInspectNode(canvasNodeId, fragmentId) {
    if (!this.domEnabled) {
      return
    }

    const backendNodeId = this.allocator.resolve(canvasNodeId, fragmentId)

    this.send({
      method: "Overlay.inspectNodeRequested",
      params: {
        backendNodeId,
      },
    })
  }

  scheduleDocumentUpdated() {
    if (this.documentUpdatedTimer != null) {
      return
    }

    this.knownDomNodeIds.clear()
    this.knownDomNodeIds.add(DOCUMENT_NODE_ID)
    this.documentUpdatedTimer = setTimeout(() => {
      this.documentUpdatedTimer = null
      this.send({ method: "DOM.documentUpdated", params: {} })
    }, 16)
  }

  scheduleLayerTreeUpdate() {
    if (!this.layerTreeEnabled) {
      return
    }

    // Mark that a refresh is wanted
    this._layerTreePendingRefresh = true

    // If already running or already scheduled, the pending flag will
    // cause a re-run after the current one finishes.
    if (this._layerTreeRunning || this.layerTreeTimer != null) {
      return
    }

    this.layerTreeTimer = setTimeout(() => {
      this.layerTreeTimer = null
      void this._runLayerTreeUpdate()
    }, 50)
  }

  async _runLayerTreeUpdate() {
    if (this._layerTreeRunning) {
      return
    }
    this._layerTreeRunning = true
    try {
      // Drain pending flag — if new events arrive during await,
      // they set the flag again and we loop.
      while (this._layerTreePendingRefresh) {
        this._layerTreePendingRefresh = false
        await this.pushLayerTree()
      }
    } finally {
      this._layerTreeRunning = false
    }
  }

  async pushLayerTree() {
    if (!this.layerTreeEnabled) {
      return
    }

    const layers = []
    const canvasIds = []
    for (const canvasNodeId of this.metadataStore.getCanvasNodeIds()) {
      const snapshot = this.treeCache.getSnapshot(canvasNodeId)
      if (snapshot.length === 0) {
        continue
      }

      canvasIds.push(canvasNodeId)

      const windowDomNodeId = this.allocator.resolve(canvasNodeId, WINDOW_FRAGMENT_ID)
      const rootLayerId = `${canvasNodeId}:root`
      const inlineLayerId = `${canvasNodeId}:inline`

      // Compute canvas bounding box
      let maxW = 0
      let maxH = 0
      for (const node of snapshot) {
        const r = (node.x ?? 0) + (node.width ?? 0)
        const b = (node.y ?? 0) + (node.height ?? 0)
        if (r > maxW) maxW = r
        if (b > maxH) maxH = b
      }

      // Root layer (non-drawing)
      layers.push({
        layerId: rootLayerId,
        backendNodeId: windowDomNodeId,
        offsetX: 0,
        offsetY: 0,
        width: maxW,
        height: maxH,
        drawsContent: false,
        paintCount: 0,
      })

      // Inline layer (all non-promoted content)
      layers.push({
        layerId: inlineLayerId,
        parentLayerId: rootLayerId,
        backendNodeId: windowDomNodeId,
        offsetX: 0,
        offsetY: 0,
        width: maxW,
        height: maxH,
        drawsContent: true,
        paintCount: 1,
      })

      // Fragment sub-layers under inline (structural visibility)
      const nodeMap = new Map()
      for (const node of snapshot) {
        nodeMap.set(node.id, node)
      }

      const ordered = []
      const visited = new Set()

      function visit(nodeId) {
        if (visited.has(nodeId)) return
        visited.add(nodeId)
        const node = nodeMap.get(nodeId)
        if (!node) return
        if (node.parentId != null && nodeMap.has(node.parentId)) {
          visit(node.parentId)
        }
        ordered.push(node)
      }

      for (const node of snapshot) {
        visit(node.id)
      }

      const nodeIds = new Set(ordered.map((n) => n.id))

      for (const node of ordered) {
        const domNodeId = this.allocator.resolve(canvasNodeId, node.id)
        const hasValidParent = node.parentId != null && nodeIds.has(node.parentId)
        const drawsContent = node.visible !== false
          && (node.width ?? 0) > 0
          && (node.height ?? 0) > 0

        layers.push({
          layerId: `${canvasNodeId}:frag:${node.id}`,
          parentLayerId: hasValidParent ? `${canvasNodeId}:frag:${node.parentId}` : inlineLayerId,
          backendNodeId: domNodeId,
          offsetX: node.x ?? 0,
          offsetY: node.y ?? 0,
          width: node.width ?? 0,
          height: node.height ?? 0,
          drawsContent,
          paintCount: drawsContent ? 1 : 0,
          ...(node.visible === false ? { invisible: true } : {}),
        })
      }

      // Promoted layers from native compositor
      const nativeLayers = await this.safeNativeRequest("snapshotLayers", { canvasNodeId }, [], 500)
      if (Array.isArray(nativeLayers)) {
        for (const nl of nativeLayers) {
          const domNodeId = this.allocator.resolve(canvasNodeId, nl.fragmentId)
          layers.push({
            layerId: `${canvasNodeId}:layer:${nl.layerKey}`,
            parentLayerId: rootLayerId,
            backendNodeId: domNodeId,
            offsetX: nl.x ?? 0,
            offsetY: nl.y ?? 0,
            width: nl.width ?? 0,
            height: nl.height ?? 0,
            drawsContent: true,
            paintCount: 1,
            ...(nl.opacity < 1 ? { invisible: false } : {}),
          })
        }
      }
    }

    // Topology signature guard
    const signature = JSON.stringify(layers.map((l) => [l.layerId, l.parentLayerId ?? null, l.width, l.height]))
    if (signature === this.lastLayerTreeSignature) {
      return
    }
    this.lastLayerTreeSignature = signature

    // ---------------------------------------------------------------
    // Build ALL caches BEFORE sending events to DevTools so that
    // makeSnapshot/replaySnapshot never hits an empty cache.
    // ---------------------------------------------------------------

    // 1. Full canvas snapshots (fallback for inline/promoted layers)
    const canvasSnapshots = new Map()
    for (const canvasNodeId of canvasIds) {
      const snap = await this.safeNativeRequest("captureCanvasFullSnapshot", { canvasNodeId }, null, 1000)
      if (snap && snap.bytes && snap.widthPx > 0 && snap.heightPx > 0) {
        canvasSnapshots.set(canvasNodeId, snap)
      }
    }

    // 2. Per-fragment bounds + isolated captures
    const layerBounds = new Map()
    const fragmentDataURLs = new Map()

    for (const canvasNodeId of canvasIds) {
      const snapshot = this.treeCache.getSnapshot(canvasNodeId)
      const nodeMap = new Map()
      const drawableFragIds = []
      for (const node of snapshot) {
        nodeMap.set(node.id, node)
      }

      for (const node of snapshot) {
        if (node.visible === false || (node.width ?? 0) <= 0 || (node.height ?? 0) <= 0) {
          continue
        }
        let absX = node.x ?? 0
        let absY = node.y ?? 0
        let pid = node.parentId
        while (pid != null) {
          const parent = nodeMap.get(pid)
          if (!parent) break
          absX += parent.x ?? 0
          absY += parent.y ?? 0
          pid = parent.parentId
        }
        layerBounds.set(`${canvasNodeId}:frag:${node.id}`, {
          canvasNodeId,
          x: absX,
          y: absY,
          width: node.width ?? 0,
          height: node.height ?? 0,
        })
        drawableFragIds.push(node.id)
      }

      if (drawableFragIds.length > 0) {
        const fragCaptures = await this.safeNativeRequest("captureAllFragmentsIsolated", {
          canvasNodeId,
          fragmentIds: drawableFragIds,
        }, null, 5000)
        if (fragCaptures && typeof fragCaptures === "object") {
          for (const [fid, dataURL] of Object.entries(fragCaptures)) {
            if (typeof dataURL === "string") {
              fragmentDataURLs.set(`${canvasNodeId}:frag:${fid}`, dataURL)
            }
          }
        }
      }
    }

    // 3. Inline + promoted layer bounds
    for (const layer of layers) {
      if (layer.drawsContent && !layerBounds.has(layer.layerId)) {
        const canvasNodeId = Number(layer.layerId.split(":")[0])
        layerBounds.set(layer.layerId, {
          canvasNodeId,
          x: layer.offsetX,
          y: layer.offsetY,
          width: layer.width,
          height: layer.height,
        })
      }
    }

    // 4. Atomically swap caches
    this._canvasSnapshots = canvasSnapshots
    this._layerBounds = layerBounds
    this._fragmentDataURLs = fragmentDataURLs

    // ---------------------------------------------------------------
    // NOW notify DevTools — caches are ready for immediate queries
    // ---------------------------------------------------------------

    this.send({
      method: "LayerTree.layerTreeDidChange",
      params: { layers },
    })

    for (const layer of layers) {
      if (layer.drawsContent && layer.width > 0 && layer.height > 0) {
        this.send({
          method: "LayerTree.layerPainted",
          params: {
            layerId: layer.layerId,
            clip: { x: layer.offsetX, y: layer.offsetY, width: layer.width, height: layer.height },
          },
        })
      }
    }
  }

  handleDevtoolsEvent(event) {
    if (!this.domEnabled) {
      return
    }

    switch (event.type) {
      case "canvas-added":
      case "canvas-removed": {
        this.scheduleDocumentUpdated()
        this.scheduleLayerTreeUpdate()
        return
      }
      case "node-created":
      case "node-inserted":
      case "node-removed":
      case "node-destroyed": {
        this.scheduleDocumentUpdated()
        this.scheduleLayerTreeUpdate()
        return
      }
      case "text-changed": {
        const domNodeId = this.allocator.resolve(event.canvasNodeId, event.fragmentId)
        if (this.knownDomNodeIds.has(domNodeId)) {
          this.send({
            method: "DOM.characterDataModified",
            params: {
              nodeId: domNodeId,
              characterData: event.value ?? "",
            },
          })
        }
        return
      }
      case "prop-changed": {
        const domNodeId = this.allocator.resolve(event.canvasNodeId, event.fragmentId)
        if (this.knownDomNodeIds.has(domNodeId) && isVisibleInlineAttributeName(event.key)) {
          this.scheduleDocumentUpdated()
        }
        return
      }
    }
  }

  // --- CDP request handler ---

  async handleRequest(method, params) {
    switch (method) {
      case "Schema.getDomains": {
        return { domains: [...KNOWN_DOMAINS] }
      }
      case "DOM.enable": {
        this.domEnabled = true
        this.knownDomNodeIds.clear()
        this.knownDomNodeIds.add(DOCUMENT_NODE_ID)
        return {}
      }
      case "DOM.disable": {
        this.domEnabled = false
        this.knownDomNodeIds.clear()
        this.knownDomNodeIds.add(DOCUMENT_NODE_ID)
        return {}
      }
      case "DOM.getDocument": {

        const depth = typeof params.depth === "number" ? params.depth : 2
        const root = this.buildDocumentNode(depth)
        this.rememberKnownNode(root)
        return { root }
      }
      case "DOM.requestChildNodes": {

        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID
        const depth = typeof params.depth === "number" ? params.depth : 1

        const children = this.buildChildNodes(nodeId, depth)
        this.rememberKnownNodes(children)
        this.send({
          method: "DOM.setChildNodes",
          params: {
            parentId: nodeId,
            nodes: children,
          },
        })
        return {}
      }
      case "DOM.describeNode": {

        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID
        const depth = typeof params.depth === "number" ? params.depth : 0
        const node = this.buildNodeDescription(nodeId, depth)
        this.rememberKnownNode(node)
        return { node }
      }
      case "DOM.getAttributes": {

        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        if (nodeId === DOCUMENT_NODE_ID) {
          return { attributes: [] }
        }

        const resolved = this.allocator.lookup(nodeId)
        if (!resolved) {
          return { attributes: [] }
        }

        if (resolved.fragmentId === WINDOW_FRAGMENT_ID) {
          return { attributes: [] }
        }

        const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
        if (!snap) {
          throw new Error(`Unknown DOM node ${nodeId}`)
        }

        return { attributes: attributesForSnapshotNode(snap) }
      }
      case "DOM.getBoxModel": {

        return await this.handleGetBoxModel(params)
      }
      case "DOM.getNodeForLocation": {
        return await this.handleGetNodeForLocation(params)
      }
      case "DOM.resolveNode": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        const resolved = this.allocator.lookup(nodeId)
        if (!resolved) {
          throw new Error(`Unknown DOM node ${nodeId}`)
        }

        if (resolved.fragmentId === WINDOW_FRAGMENT_ID) {
          return {
            object: {
              type: "object",
              subtype: "node",
              className: "window",
              description: "<window>",
              objectId: `${SYNTHETIC_OBJECT_PREFIX}window:${resolved.canvasNodeId}`,
            },
          }
        }

        const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
        const tag = snap?.tag ?? "unknown"
        return { object: buildFragmentRemoteObject(resolved.canvasNodeId, resolved.fragmentId, tag) }
      }
      case "DOM.requestNode": {
        const parsed = parseSyntheticObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
        if (!parsed) {
          throw new Error("Unsupported object id")
        }

        const nodeId = this.allocator.resolve(parsed.canvasNodeId, parsed.fragmentId)
        this.knownDomNodeIds.add(nodeId)
        return { nodeId }
      }
      case "DOM.pushNodesByBackendIdsToFrontend": {

        const backendNodeIds = Array.isArray(params.backendNodeIds)
          ? params.backendNodeIds.filter((value) => typeof value === "number")
          : []

        return {
          nodeIds: backendNodeIds.map((backendNodeId) => {
            const resolved = this.allocator.lookup(backendNodeId)
            if (!resolved) {
              return 0
            }

            if (resolved.fragmentId === WINDOW_FRAGMENT_ID) {
              this.knownDomNodeIds.add(backendNodeId)
              return backendNodeId
            }

            const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
            if (!snap) {
              return 0
            }

            return this.materializeNodePath(resolved.canvasNodeId, resolved.fragmentId)
          }),
        }
      }
      case "DOM.setNodeStackTracesEnabled": {
        return {}
      }
      case "DOM.getNodeStackTraces": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID
        if (nodeId === DOCUMENT_NODE_ID) {
          return {}
        }

        const resolved = this.allocator.lookup(nodeId)
        if (!resolved || resolved.fragmentId === WINDOW_FRAGMENT_ID) {
          return {}
        }

        const meta = this.metadataStore.getMetadata(resolved.canvasNodeId, resolved.fragmentId)
        const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
        const kind = snap?.tag ?? "unknown"
        const creation = creationStackTrace(meta?.owner ?? null, meta?.source ?? null, kind)
        return creation ? { creation } : {}
      }
      case "DOM.setInspectedNode": {
        return {}
      }
      case "CSS.enable":
      case "CSS.disable":
      case "Overlay.enable":
      case "Overlay.disable": {
        return {}
      }
      case "Overlay.setInspectMode": {
        const mode = typeof params.mode === "string" ? params.mode : "none"
        if (mode === "none") {
          await this.safeNativeRequest("setInspectMode", { enabled: false })
          await this.safeNativeRequest("clearHighlight", {})
          return {}
        }

        await this.safeNativeRequest("setInspectMode", { enabled: true })
        return {}
      }
      case "Overlay.highlightNode": {
        const requestedNodeId = typeof params.nodeId === "number"
          ? params.nodeId
          : typeof params.backendNodeId === "number"
            ? params.backendNodeId
            : DOCUMENT_NODE_ID

        if (requestedNodeId === DOCUMENT_NODE_ID) {
          await this.clearAllHighlights()
          return {}
        }

        const resolved = this.allocator.lookup(requestedNodeId)
        if (!resolved) {
          await this.clearAllHighlights()
          return {}
        }

        if (resolved.fragmentId === WINDOW_FRAGMENT_ID) {
          await this.clearAllHighlights()
          return {}
        }

        await this.safeNativeRequest("highlightFragment", {
          canvasNodeId: resolved.canvasNodeId,
          fragmentId: resolved.fragmentId,
        })
        return {}
      }
      case "Overlay.hideHighlight": {
        await this.clearAllHighlights()
        return {}
      }
      case "CSS.getComputedStyleForNode": {

        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        const resolved = this.allocator.lookup(nodeId)
        if (!resolved || resolved.fragmentId === WINDOW_FRAGMENT_ID) {
          return { computedStyle: [] }
        }

        const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
        if (!snap) {
          return { computedStyle: [] }
        }

        return { computedStyle: styleEntriesForSnapshotNode(snap) }
      }
      case "CSS.getMatchedStylesForNode": {

        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        const resolved = this.allocator.lookup(nodeId)
        if (!resolved || resolved.fragmentId === WINDOW_FRAGMENT_ID) {
          return {
            inlineStyle: cssStyle([]),
            matchedCSSRules: [],
            pseudoElements: [],
            inherited: [],
            cssKeyframesRules: [],
          }
        }

        const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
        if (!snap) {
          return {
            inlineStyle: cssStyle([]),
            matchedCSSRules: [],
            pseudoElements: [],
            inherited: [],
            cssKeyframesRules: [],
          }
        }

        return {
          inlineStyle: cssStyle(styleEntriesForSnapshotNode(snap)),
          matchedCSSRules: [],
          pseudoElements: [],
          inherited: [],
          cssKeyframesRules: [],
        }
      }
      case "Runtime.getProperties": {
        const objectId = typeof params.objectId === "string" ? params.objectId : undefined
        const parsed = parseSyntheticObjectId(objectId)
        if (!parsed) {
          throw new Error("Unsupported object id")
        }

        if (parsed.type === "window") {
          return { result: [], internalProperties: [] }
        }



        const snap = this.treeCache.getNode(parsed.canvasNodeId, parsed.fragmentId)
        if (!snap) {
          throw new Error(`Unknown fragment ${parsed.canvasNodeId}:${parsed.fragmentId}`)
        }

        const meta = this.metadataStore.getMetadata(parsed.canvasNodeId, parsed.fragmentId)
        return {
          result: nodeProperties(snap, meta),
          internalProperties: [],
        }
      }
      case "Runtime.releaseObject":
      case "Runtime.releaseObjectGroup": {
        const objectId = typeof params.objectId === "string" ? params.objectId : undefined
        const parsed = parseSyntheticObjectId(objectId)
        if (parsed != null) {
          return {}
        }

        // releaseObjectGroup may not have objectId
        if (method === "Runtime.releaseObjectGroup") {
          return {}
        }

        throw new Error("Unsupported object id")
      }
      // LayerTree domain
      case "LayerTree.enable": {
        this.layerTreeEnabled = true
        this.scheduleLayerTreeUpdate()
        return {}
      }
      case "LayerTree.disable": {
        this.layerTreeEnabled = false
        return {}
      }
      case "LayerTree.compositingReasons": {
        const layerId = typeof params.layerId === "string" ? params.layerId : ""
        // Canvas-level layers: "canvasId:canvas" or "canvasId:root"
        const match = /^(\d+):/.exec(layerId)
        if (!match) {
          return { compositingReasons: [], compositingReasonIds: [] }
        }
        return {
          compositingReasons: ["canvas rendering surface"],
          compositingReasonIds: [],
        }
      }
      case "LayerTree.makeSnapshot": {
        const layerId = typeof params.layerId === "string" ? params.layerId : ""
        const snapshotId = `snapshot:${this.nextSnapshotId++}`

        // Check pre-cached fragment captures first (zero round-trip)
        const cachedDataURL = this._fragmentDataURLs?.get(layerId)
        if (cachedDataURL) {
          this.layerSnapshots.set(snapshotId, { directDataURL: cachedDataURL })
          return { snapshotId }
        }

        const bounds = this._layerBounds?.get(layerId)
        if (bounds && bounds.width > 0 && bounds.height > 0) {
          const canvasSnapshot = this._canvasSnapshots?.get(bounds.canvasNodeId) ?? null
          this.layerSnapshots.set(snapshotId, { ...bounds, canvasSnapshot })
        }

        return { snapshotId }
      }
      case "LayerTree.replaySnapshot": {
        const snapshotId = typeof params.snapshotId === "string" ? params.snapshotId : ""
        const info = this.layerSnapshots.get(snapshotId)
        if (!info) {
          return { dataURL: TRANSPARENT_1X1_PNG }
        }

        // Direct data URL from isolated fragment capture
        if (info.directDataURL) {
          return { dataURL: info.directDataURL }
        }

        if (info.width <= 0 || info.height <= 0) {
          return { dataURL: TRANSPARENT_1X1_PNG }
        }

        if (info.canvasSnapshot) {
          const dataURL = cropAndEncode(info.canvasSnapshot, info.x, info.y, info.width, info.height)
          if (dataURL) {
            return { dataURL }
          }
        }

        // Fallback: capture on demand
        const snap = await this.safeNativeRequest("captureCanvasFullSnapshot", { canvasNodeId: info.canvasNodeId }, null, 1000)
        if (snap && snap.bytes && snap.widthPx > 0 && snap.heightPx > 0) {
          const dataURL = cropAndEncode(snap, info.x, info.y, info.width, info.height)
          if (dataURL) {
            return { dataURL }
          }
        }

        return { dataURL: TRANSPARENT_1X1_PNG }
      }
      case "LayerTree.profileSnapshot": {
        return { timings: [] }
      }
      case "LayerTree.snapshotCommandLog": {
        return { commandLog: [] }
      }
      case "LayerTree.releaseSnapshot": {
        const snapshotId = typeof params.snapshotId === "string" ? params.snapshotId : ""
        this.layerSnapshots.delete(snapshotId)
        return {}
      }
      // Animation domain
      case "Animation.enable":
      case "Animation.disable": {
        return {}
      }
      case "Animation.getPlaybackRate": {
        return { playbackRate: 1.0 }
      }
      case "Animation.setPlaybackRate":
      case "Animation.setPaused":
      case "Animation.seekAnimations":
      case "Animation.releaseAnimations":
      case "Animation.resolveAnimation": {
        return {}
      }
      default:
        throw new Error(`Unsupported synthetic method ${method}`)
    }
  }

  // --- DOM tree building from native snapshots ---

  buildDocumentNode(depth) {
    const canvasNodeIds = [...this.metadataStore.getCanvasNodeIds()]
    const windowChildren = depth > 0
      ? canvasNodeIds.map((canvasNodeId) => this.buildWindowNode(canvasNodeId, depth - 1))
      : undefined

    return {
      nodeId: DOCUMENT_NODE_ID,
      backendNodeId: DOCUMENT_NODE_ID,
      nodeType: 9,
      nodeName: "#document",
      localName: "",
      nodeValue: "",
      childNodeCount: canvasNodeIds.length,
      children: windowChildren,
      documentURL: "qt-solid://app",
      baseURL: "qt-solid://app",
      xmlVersion: "",
    }
  }

  buildWindowNode(canvasNodeId, depth) {
    const windowDomNodeId = this.allocator.resolve(canvasNodeId, WINDOW_FRAGMENT_ID)
    const rootChildIds = this.treeCache.getRootChildIds(canvasNodeId)
    const children = depth > 0
      ? rootChildIds.map((fragId) => this.buildFragmentNode(canvasNodeId, fragId, depth - 1))
      : undefined

    return {
      nodeId: windowDomNodeId,
      backendNodeId: windowDomNodeId,
      nodeType: 1,
      nodeName: "WINDOW",
      localName: "window",
      nodeValue: "",
      attributes: [],
      childNodeCount: rootChildIds.length,
      children,
    }
  }

  buildFragmentNode(canvasNodeId, fragmentId, depth) {
    const domNodeId = this.allocator.resolve(canvasNodeId, fragmentId)
    const snap = this.treeCache.getNode(canvasNodeId, fragmentId)

    if (!snap) {
      return {
        nodeId: domNodeId,
        backendNodeId: domNodeId,
        nodeType: 1,
        nodeName: "UNKNOWN",
        localName: "unknown",
        nodeValue: "",
        attributes: [],
        childNodeCount: 0,
      }
    }

    if (snap.tag === "Text" || snap.tag === "#text") {
      const textValue = snap.props?.text ?? ""
      return {
        nodeId: domNodeId,
        backendNodeId: domNodeId,
        nodeType: 3,
        nodeName: "#text",
        localName: "",
        nodeValue: textValue,
        childNodeCount: 0,
      }
    }

    const childIds = snap.childIds ?? []
    const children = depth > 0
      ? childIds.map((childId) => this.buildFragmentNode(canvasNodeId, childId, depth - 1))
      : undefined

    return {
      nodeId: domNodeId,
      backendNodeId: domNodeId,
      nodeType: 1,
      nodeName: snap.tag.toUpperCase(),
      localName: snap.tag,
      nodeValue: "",
      attributes: attributesForSnapshotNode(snap),
      childNodeCount: childIds.length,
      children,
    }
  }

  buildNodeDescription(nodeId, depth) {
    if (nodeId === DOCUMENT_NODE_ID) {
      return this.buildDocumentNode(depth)
    }

    const resolved = this.allocator.lookup(nodeId)
    if (!resolved) {
      throw new Error(`Unknown DOM node ${nodeId}`)
    }

    if (resolved.fragmentId === WINDOW_FRAGMENT_ID) {
      return this.buildWindowNode(resolved.canvasNodeId, depth)
    }

    return this.buildFragmentNode(resolved.canvasNodeId, resolved.fragmentId, depth)
  }

  buildChildNodes(parentNodeId, depth) {
    if (parentNodeId === DOCUMENT_NODE_ID) {
      const canvasNodeIds = [...this.metadataStore.getCanvasNodeIds()]
      return canvasNodeIds.map((canvasNodeId) => this.buildWindowNode(canvasNodeId, depth - 1))
    }

    const resolved = this.allocator.lookup(parentNodeId)
    if (!resolved) {
      return []
    }

    if (resolved.fragmentId === WINDOW_FRAGMENT_ID) {
      const rootChildIds = this.treeCache.getRootChildIds(resolved.canvasNodeId)
      return rootChildIds.map((fragId) => this.buildFragmentNode(resolved.canvasNodeId, fragId, depth - 1))
    }

    const snap = this.treeCache.getNode(resolved.canvasNodeId, resolved.fragmentId)
    if (!snap) {
      return []
    }

    return (snap.childIds ?? []).map((childId) => this.buildFragmentNode(resolved.canvasNodeId, childId, depth - 1))
  }

  // --- Box model ---

  async handleGetBoxModel(params) {
    const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

    if (nodeId === DOCUMENT_NODE_ID) {
      return this.emptyBoxModel()
    }

    const resolved = this.allocator.lookup(nodeId)
    if (!resolved || resolved.fragmentId === WINDOW_FRAGMENT_ID) {
      return this.emptyBoxModel()
    }

    const bounds = await this.getFragmentBounds(resolved.canvasNodeId, resolved.fragmentId)
    return this.boxModelFromBounds(bounds)
  }

  async handleGetNodeForLocation(params) {
    const screenX = typeof params.x === "number" ? params.x : 0
    const screenY = typeof params.y === "number" ? params.y : 0

    if (!this.isPaused()) {
      for (const canvasNodeId of this.metadataStore.getCanvasNodeIds()) {
        const fragmentId = await this.nativeBridge.safeRequest(
          "fragmentHitTest",
          { canvasNodeId, x: screenX, y: screenY },
          null,
        )
        if (fragmentId != null && typeof fragmentId === "number") {
          const domNodeId = this.allocator.resolve(canvasNodeId, fragmentId)
          this.knownDomNodeIds.add(domNodeId)
          return {
            backendNodeId: domNodeId,
            nodeId: domNodeId,
            frameId: "qt-solid-frame",
          }
        }
      }

      // Fallback: try legacy getNodeAtPoint
      const nodeAtPoint = await this.nativeBridge.safeRequest("getNodeAtPoint", { screenX, screenY }, null)
      if (nodeAtPoint != null && typeof nodeAtPoint === "number") {
        // Legacy node — find a canvas that might own it
        for (const canvasNodeId of this.metadataStore.getCanvasNodeIds()) {
          const snap = this.treeCache.getNode(canvasNodeId, nodeAtPoint)
          if (snap) {
            const domNodeId = this.allocator.resolve(canvasNodeId, nodeAtPoint)
            this.knownDomNodeIds.add(domNodeId)
            return {
              backendNodeId: domNodeId,
              nodeId: domNodeId,
              frameId: "qt-solid-frame",
            }
          }
        }
      }
    }

    // Fallback — return first window node or document
    const canvasNodeIds = [...this.metadataStore.getCanvasNodeIds()]
    if (canvasNodeIds.length > 0) {
      const windowDomNodeId = this.allocator.resolve(canvasNodeIds[0], WINDOW_FRAGMENT_ID)
      return {
        backendNodeId: windowDomNodeId,
        nodeId: windowDomNodeId,
        frameId: "qt-solid-frame",
      }
    }

    return {
      backendNodeId: DOCUMENT_NODE_ID,
      nodeId: DOCUMENT_NODE_ID,
      frameId: "qt-solid-frame",
    }
  }

  emptyBoxModel() {
    const content = boxQuad(0, 0, 0, 0)
    return {
      model: {
        width: 0,
        height: 0,
        content,
        padding: content,
        border: content,
        margin: content,
      },
    }
  }

  boxModelFromBounds(bounds) {
    const content = boxQuad(bounds.screenX, bounds.screenY, bounds.width, bounds.height)
    return {
      model: {
        width: bounds.width,
        height: bounds.height,
        content,
        padding: content,
        border: content,
        margin: content,
      },
    }
  }

  async getFragmentBounds(canvasNodeId, fragmentId) {
    if (this.isPaused()) {
      return { visible: false, screenX: 0, screenY: 0, width: 0, height: 0 }
    }

    const bounds = await this.nativeBridge.safeRequest(
      "getFragmentBounds",
      { canvasNodeId, fragmentId },
      null,
    )
    if (bounds) {
      return bounds
    }

    return { visible: false, screenX: 0, screenY: 0, width: 0, height: 0 }
  }

  async clearAllHighlights() {
    for (const canvasNodeId of this.metadataStore.getCanvasNodeIds()) {
      await this.safeNativeRequest("clearFragmentHighlight", { canvasNodeId })
    }
    // Also try legacy clear
    await this.safeNativeRequest("clearHighlight", {})
  }

  async safeNativeRequest(method, params, fallback = {}, timeoutMs = 150) {
    if (this.isPaused()) {
      return fallback
    }

    try {
      return await this.nativeBridge.request(method, params, timeoutMs)
    } catch {
      return fallback
    }
  }

  // --- Known node tracking ---

  rememberKnownNode(node) {
    const nodeId = typeof node.nodeId === "number" ? node.nodeId : null
    if (nodeId != null) {
      this.knownDomNodeIds.add(nodeId)
    }

    const children = Array.isArray(node.children) ? node.children : []
    for (const child of children) {
      if (child && typeof child === "object") {
        this.rememberKnownNode(child)
      }
    }
  }

  rememberKnownNodes(nodes) {
    for (const node of nodes) {
      this.rememberKnownNode(node)
    }
  }

  materializeNodePath(canvasNodeId, fragmentId) {
    // Walk up the snapshot tree to find the path from root to this fragment
    const path = []
    let currentFragId = fragmentId

    while (currentFragId != null) {
      path.push(currentFragId)
      const snap = this.treeCache.getNode(canvasNodeId, currentFragId)
      currentFragId = snap?.parentId ?? null
    }

    path.reverse()

    // Start from the window node
    const windowDomNodeId = this.allocator.resolve(canvasNodeId, WINDOW_FRAGMENT_ID)
    if (!this.knownDomNodeIds.has(windowDomNodeId)) {
      // Materialize document → window
      const windowChildren = this.buildChildNodes(DOCUMENT_NODE_ID, 0)
      this.rememberKnownNodes(windowChildren)
      this.send({
        method: "DOM.setChildNodes",
        params: {
          parentId: DOCUMENT_NODE_ID,
          nodes: windowChildren,
        },
      })
    }

    let parentDomNodeId = windowDomNodeId
    for (const currentFragmentId of path) {
      const currentDomNodeId = this.allocator.resolve(canvasNodeId, currentFragmentId)
      if (!this.knownDomNodeIds.has(currentDomNodeId)) {
        const children = this.buildChildNodes(parentDomNodeId, 0)
        this.rememberKnownNodes(children)
        this.send({
          method: "DOM.setChildNodes",
          params: {
            parentId: parentDomNodeId,
            nodes: children,
          },
        })
      }
      parentDomNodeId = currentDomNodeId
    }

    return this.allocator.resolve(canvasNodeId, fragmentId)
  }

  send(payload) {
    this.sendPayload(payload)
  }
}

// ---------------------------------------------------------------------------
// Routing helpers
// ---------------------------------------------------------------------------

function isSyntheticRuntimeMethod(method, params) {
  const objectId = typeof params.objectId === "string" ? params.objectId : undefined
  if (method === "Runtime.getProperties" || method === "Runtime.releaseObject") {
    return objectId?.startsWith(SYNTHETIC_OBJECT_PREFIX) || false
  }

  return method === "Runtime.releaseObjectGroup"
}

function isSyntheticMethod(method, params) {
  return method.startsWith("DOM.")
    || method.startsWith("CSS.")
    || method.startsWith("Overlay.")
    || method.startsWith("LayerTree.")
    || method.startsWith("Animation.")
    || isSyntheticRuntimeMethod(method, params)
}

// ---------------------------------------------------------------------------
// WorkerClientConnection
// ---------------------------------------------------------------------------

class WorkerClientConnection {
  constructor(connectionId, socket, allocator, metadataStore, treeCache, nativeBridge) {
    this.connectionId = connectionId
    this.socket = socket
    this.session = new inspector.Session()
    this.paused = false

    const isPaused = () => this.paused

    this.backend = new SyntheticBackend(
      allocator,
      metadataStore,
      treeCache,
      nativeBridge,
      (payload) => this.send(payload),
      isPaused,
    )
    this.treeCache = treeCache
    this.session.connectToMainThread()
    this.session.on("inspectorNotification", (message) => {
      if (message.method === "Debugger.paused") {
        this.paused = true
      } else if (message.method === "Debugger.resumed") {
        this.paused = false
      }

      this.send(message)
    })
  }

  dispose() {
    this.backend.dispose()
    this.session.disconnect()
  }

  handleDevtoolsEvent(event) {
    this.backend.handleDevtoolsEvent(event)
  }

  async handleMessage(raw) {
    const payload = JSON.parse(raw.toString())
    if (!payload.method || payload.id == null) {
      return
    }

    try {
      const result = await this.dispatch(payload.method, payload.params ?? {})
      this.send({ id: payload.id, result })
    } catch (error) {
      this.send({
        id: payload.id,
        error: {
          code: -32000,
          message: error instanceof Error ? error.message : String(error),
        },
      })
    }
  }

  async dispatch(method, params) {
    if (method === "Schema.getDomains") {
      const forwarded = await withTimeout(postInspector(this.session, method, params).catch(() => ({ domains: [] })), 500, "Timed out waiting for Schema.getDomains").catch(() => ({ domains: [] }))
      const domains = Array.isArray(forwarded.domains) ? forwarded.domains : []
      const extraDomains = KNOWN_DOMAINS.filter(
        (extra) => !domains.some((domain) => domain && typeof domain === "object" && domain.name === extra.name),
      )
      return { domains: [...domains, ...extraDomains] }
    }

    if (isSyntheticMethod(method, params)) {
      return await this.backend.handleRequest(method, params)
    }

    return await postInspector(this.session, method, params)
  }

  send(payload) {
    this.socket.send(JSON.stringify(payload))
  }
}

// ---------------------------------------------------------------------------
// run()
// ---------------------------------------------------------------------------

async function run() {
  const { port } = workerData
  const allocator = new NodeIdAllocator()
  const metadataStore = new MetadataStore()
  const nativeBridge = new NativeBridge()
  const treeCache = new FragmentTreeCache()

  const httpServer = createServer((request, response) => {
    const url = request.url ?? "/"
    if (url === "/json/version") {
      sendJson(response, {
        Browser: "qt-solid-spike",
        "Protocol-Version": "1.3",
        "User-Agent": `Node ${process.version}`,
      })
      return
    }

    if (url === "/json" || url === "/json/list") {
      sendJson(response, [targetDescriptor(port)])
      return
    }

    response.writeHead(404)
    response.end("not found")
  })

  const websocketServer = new WebSocketServer({ noServer: true })
  const connections = new Map()
  let nextConnectionId = 1

  parentPort?.on("message", (message) => {
    nativeBridge.receive(message)

    if (message?.type === "devtools-event") {
      metadataStore.handleDevtoolsEvent(message.event)
      for (const connection of connections.values()) {
        connection.handleDevtoolsEvent(message.event)
      }
      return
    }

    if (message?.type === "metadata-snapshot") {
      metadataStore.replaceSnapshot(message.metadata ?? [], message.canvasNodeIds ?? [])
      return
    }

    if (message?.type === "tree-snapshot" && typeof message.canvasNodeId === "number") {
      treeCache.setSnapshot(message.canvasNodeId, message.snapshot)
      return
    }

    if (message?.type === "inspect-node") {
      // rendererNodeId here is actually a fragmentId; we need to find the canvas
      const fragmentId = message.rendererNodeId
      for (const canvasNodeId of metadataStore.getCanvasNodeIds()) {
        for (const connection of connections.values()) {
          connection.backend.notifyInspectNode(canvasNodeId, fragmentId)
        }
        break // notify for first canvas that might contain it
      }
    }
  })

  websocketServer.on("connection", (socket, request) => {
    const connectionId = `${nextConnectionId++}`
    const connection = new WorkerClientConnection(connectionId, socket, allocator, metadataStore, treeCache, nativeBridge)
    connections.set(connectionId, connection)

    socket.on("message", (message) => {
      void connection.handleMessage(message)
    })
    socket.on("close", () => {
      connection.dispose()
      connections.delete(connectionId)
    })
    socket.on("error", () => {
      connection.dispose()
      connections.delete(connectionId)
    })
  })

  httpServer.on("upgrade", (request, socket, head) => {
    if (request.url !== `/devtools/page/${SYNTHETIC_TARGET_ID}`) {
      socket.destroy()
      return
    }

    websocketServer.handleUpgrade(request, socket, head, (ws) => {
      websocketServer.emit("connection", ws, request)
    })
  })

  await new Promise((resolve, reject) => {
    httpServer.once("error", reject)
    httpServer.listen(port, "127.0.0.1", () => {
      httpServer.off("error", reject)
      resolve()
    })
  })

  parentPort?.postMessage({ type: "ready", url: `http://127.0.0.1:${port}/json/list` })
}

void run()
