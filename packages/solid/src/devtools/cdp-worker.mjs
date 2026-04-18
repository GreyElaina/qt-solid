import { createServer } from "node:http"
import inspector from "node:inspector"
import { createRequire } from "node:module"
import { dirname, isAbsolute, join, resolve as resolvePath } from "node:path"
import { pathToFileURL } from "node:url"
import { parentPort, workerData } from "node:worker_threads"

const DOCUMENT_NODE_ID = 1
const DOM_NODE_OFFSET = 1_000
const COMPONENT_DOM_NODE_OFFSET = 1_000_000
const SYNTHETIC_TARGET_ID = "qt-solid-renderer"
const COMPONENTS_TARGET_ID = "qt-solid-components"
const SYNTHETIC_OBJECT_PREFIX = "qt-solid-node:"
const SYNTHETIC_COMPONENT_OBJECT_PREFIX = "qt-solid-component:"
const KNOWN_DOMAINS = [
  { name: "DOM", version: "1.3" },
  { name: "CSS", version: "1.3" },
  { name: "Overlay", version: "1.3" },
]

const require = createRequire(import.meta.url)
const wsPackageJsonPath = require.resolve("ws/package.json")
const { WebSocketServer } = require(join(dirname(wsPackageJsonPath), "index.js"))

function targetIdFor(kind) {
  return kind === "components" ? COMPONENTS_TARGET_ID : SYNTHETIC_TARGET_ID
}

function websocketPath(port, kind) {
  return `ws://127.0.0.1:${port}/devtools/page/${targetIdFor(kind)}`
}

function devtoolsFrontendUrlForWebSocketUrl(webSocketDebuggerUrl) {
  const parsed = new URL(webSocketDebuggerUrl)
  const wsTarget = `${parsed.host}${parsed.pathname}${parsed.search}`
  return `devtools://devtools/bundled/inspector.html?ws=${encodeURIComponent(wsTarget)}`
}

function targetDescriptor(port, kind) {
  const wsUrl = websocketPath(port, kind)
  const targetId = targetIdFor(kind)
  const targetName = kind === "components" ? "components" : "renderer"
  const targetUrl = kind === "components" ? "qt-solid://components" : "qt-solid://app"
  return {
    id: targetId,
    type: "page",
    title: `${process.title || "qt-solid-spike"} (${targetName})`,
    description: `Qt Solid ${targetName} target`,
    url: targetUrl,
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

function rendererNodeIdToDomNodeId(rendererNodeId) {
  return DOM_NODE_OFFSET + rendererNodeId
}

function domNodeIdToRendererNodeId(domNodeId) {
  if (domNodeId < DOM_NODE_OFFSET || domNodeId >= COMPONENT_DOM_NODE_OFFSET) {
    return null
  }

  return domNodeId - DOM_NODE_OFFSET
}

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

function styleEntriesForNode(node) {
  const entries = []
  const layoutKeys = new Set([
    "width",
    "height",
    "minWidth",
    "minHeight",
    "flexGrow",
    "flexShrink",
    "direction",
    "alignItems",
    "justifyContent",
    "gap",
    "padding",
    "visible",
  ])

  const defaultDisplay = node.kind === "#text" ? "inline" : "flex"
  entries.push({ name: "display", value: defaultDisplay })

  for (const [key, value] of Object.entries(node.props ?? {})) {
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

const HIDDEN_INLINE_ATTRIBUTE_NAMES = new Set(["kind", "source", "owner-component", "owner-path", "listeners"])

function isVisibleInlineAttributeName(name) {
  return !HIDDEN_INLINE_ATTRIBUTE_NAMES.has(name)
}

function attributesForNode(node) {
  const attributes = []

  if (node.text != null && node.kind === "#text") {
    attributes.push("text", node.text)
  }

  for (const [key, value] of Object.entries(node.props ?? {})) {
    if (!isVisibleInlineAttributeName(key)) {
      continue
    }

    attributes.push(key, stringifyValue(value))
  }

  return attributes
}

function attributeValueForNode(node, name) {
  if (!isVisibleInlineAttributeName(name)) {
    return null
  }

  if (Object.hasOwn(node.props ?? {}, name)) {
    return stringifyValue(node.props[name])
  }

  return null
}

function componentNodeName(node) {
  return node.role === "component" ? (node.componentName ?? "Component") : node.rendererKind
}

function componentAttributesForNode(node) {
  const attributes = []

  if (node.source) {
    attributes.push("source", formatSourceMetadata(node.source))
  }

  return attributes
}

function componentNodeDescription(store, mappedTree, nodeId, depth = 0) {
  if (nodeId === DOCUMENT_NODE_ID) {
    const children = mappedTree.rootChildNodeIds.map((childId) => componentNodeDescription(store, mappedTree, childId, depth))
    return {
      nodeId: DOCUMENT_NODE_ID,
      backendNodeId: DOCUMENT_NODE_ID,
      nodeType: 9,
      nodeName: "#document",
      localName: "",
      nodeValue: "",
      childNodeCount: children.length,
      children,
      documentURL: "qt-solid://components",
      baseURL: "qt-solid://components",
      xmlVersion: "",
    }
  }

  const node = mappedTree.nodes.get(nodeId)
  if (!node) {
    throw new Error(`Unknown components DOM node ${nodeId}`)
  }

  const children = depth > 0
    ? node.childNodeIds.map((childId) => componentNodeDescription(store, mappedTree, childId, depth - 1))
    : undefined

  const name = componentNodeName(node)
  return {
    nodeId,
    backendNodeId: nodeId,
    nodeType: 1,
    nodeName: name,
    localName: name,
    nodeValue: "",
    attributes: componentAttributesForNode(node),
    childNodeCount: node.childNodeIds.length,
    children,
  }
}

function buildNodeRemoteObject(node) {
  return {
    type: "object",
    subtype: "node",
    className: node.kind,
    description: `<${node.kind}>`,
    objectId: `${SYNTHETIC_OBJECT_PREFIX}${node.id}`,
  }
}

function buildComponentRemoteObject(node) {
  const name = componentNodeName(node)
  return {
    type: "object",
    subtype: "node",
    className: name,
    description: `<${name}>`,
    objectId: `${SYNTHETIC_COMPONENT_OBJECT_PREFIX}${node.domNodeId}`,
  }
}

function parseSyntheticObjectId(objectId) {
  if (!objectId?.startsWith(SYNTHETIC_OBJECT_PREFIX)) {
    return null
  }

  const numeric = Number(objectId.slice(SYNTHETIC_OBJECT_PREFIX.length))
  return Number.isFinite(numeric) ? numeric : null
}

function parseSyntheticComponentObjectId(objectId) {
  if (!objectId?.startsWith(SYNTHETIC_COMPONENT_OBJECT_PREFIX)) {
    return null
  }

  const numeric = Number(objectId.slice(SYNTHETIC_COMPONENT_OBJECT_PREFIX.length))
  return Number.isFinite(numeric) ? numeric : null
}

function nodeDescription(store, nodeId, depth = 0) {
  const snapshot = store.snapshot()
  if (nodeId === DOCUMENT_NODE_ID) {
    const rootNode = snapshot.rootId == null ? [] : [nodeDescription(store, rendererNodeIdToDomNodeId(snapshot.rootId), depth)]
    return {
      nodeId: DOCUMENT_NODE_ID,
      backendNodeId: DOCUMENT_NODE_ID,
      nodeType: 9,
      nodeName: "#document",
      localName: "",
      nodeValue: "",
      childNodeCount: rootNode.length,
      children: rootNode,
      documentURL: "qt-solid://app",
      baseURL: "qt-solid://app",
      xmlVersion: "",
    }
  }

  const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
  const node = rendererNodeId == null ? undefined : store.getNode(rendererNodeId)
  if (!node) {
    throw new Error(`Unknown DOM node ${nodeId}`)
  }

  if (node.kind === "#text") {
    return {
      nodeId,
      backendNodeId: nodeId,
      nodeType: 3,
      nodeName: "#text",
      localName: "",
      nodeValue: node.text ?? "",
      childNodeCount: 0,
    }
  }

  const children = depth > 0
    ? node.childIds.map((childId) => nodeDescription(store, rendererNodeIdToDomNodeId(childId), depth - 1))
    : undefined

  return {
    nodeId,
    backendNodeId: nodeId,
    nodeType: 1,
    nodeName: node.kind.toUpperCase(),
    localName: node.kind,
    nodeValue: "",
    attributes: attributesForNode(node),
    childNodeCount: node.childIds.length,
    children,
  }
}

function childNodeDescriptions(store, parentNodeId) {
  if (parentNodeId === DOCUMENT_NODE_ID) {
    const snapshot = store.snapshot()
    return snapshot.rootId == null ? [] : [nodeDescription(store, rendererNodeIdToDomNodeId(snapshot.rootId), 0)]
  }

  const rendererNodeId = domNodeIdToRendererNodeId(parentNodeId)
  const node = rendererNodeId == null ? undefined : store.getNode(rendererNodeId)
  if (!node) {
    return []
  }

  return node.childIds.map((childId) => nodeDescription(store, rendererNodeIdToDomNodeId(childId), 0))
}

function nodeProperties(node) {
  const sourceLocation = node.source ? serializeSourceLocation(node.source) : null
  const nodeCreationFrames = creationFrames(node.owner, node.source, node.kind)

  return [
    {
      name: "kind",
      value: { type: "string", value: node.kind },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "source",
      value: { type: "string", value: node.source ? formatSourceMetadata(node.source) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceFileName",
      value: { type: "string", value: node.source?.fileName ?? "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceLineNumber",
      value: { type: "number", value: node.source?.lineNumber ?? 0 },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "sourceColumnNumber",
      value: { type: "number", value: node.source?.columnNumber ?? 0 },
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
      value: { type: "string", value: node.owner ? ownerComponentName(node.owner) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "ownerPath",
      value: { type: "string", value: node.owner ? formatOwnerPath(node.owner) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "ownerStack",
      value: {
        type: "object",
        subtype: "array",
        value: node.owner ? serializeOwnerStack(node.owner) : [],
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
      name: "text",
      value: { type: "string", value: node.text ?? "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "props",
      value: {
        type: "object",
        subtype: "map",
        value: { ...(node.props ?? {}) },
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "listeners",
      value: {
        type: "object",
        subtype: "array",
        value: [...(node.listeners ?? [])],
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
        value: [...node.childIds],
      },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
  ]
}

function componentNodeProperties(node) {
  const sourceLocation = node.source ? serializeSourceLocation(node.source) : null
  return [
    {
      name: "kind",
      value: { type: "string", value: node.role === "component" ? "component" : "host" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "componentName",
      value: { type: "string", value: node.componentName ?? "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "componentPath",
      value: { type: "string", value: node.componentPath },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "rendererNodeId",
      value: { type: "number", value: node.rendererNodeId },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "rendererKind",
      value: { type: "string", value: node.rendererKind },
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
      name: "sourceUrl",
      value: { type: "string", value: node.source ? sourceMetadataUrl(node.source) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
    {
      name: "frameKind",
      value: { type: "string", value: node.source ? sourceMetadataFrameKind(node.source) : "" },
      enumerable: true,
      configurable: true,
      isOwn: true,
    },
  ]
}

class MirrorInspectorStore {
  constructor() {
    this.currentSnapshot = {
      rootId: null,
      nodes: [],
      revision: 0,
    }
    this.nodes = new Map()
    this.listeners = new Set()
  }

  replaceSnapshot(snapshot, mutation) {
    this.currentSnapshot = snapshot ?? {
      rootId: null,
      nodes: [],
      revision: 0,
    }
    this.nodes = new Map(this.currentSnapshot.nodes.map((node) => [node.id, node]))

    if (!mutation) {
      return
    }

    for (const listener of this.listeners) {
      listener(mutation)
    }
  }

  snapshot() {
    return this.currentSnapshot
  }

  getNode(nodeId) {
    return this.nodes.get(nodeId)
  }

  subscribe(listener) {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }
}

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
}

class SyntheticBackend {
  constructor(targetKind, store, nativeBridge, sendPayload, isPaused) {
    this.targetKind = targetKind
    this.store = store
    this.nativeBridge = nativeBridge
    this.sendPayload = sendPayload
    this.isPaused = isPaused
    this.domEnabled = false
    this.knownDomNodeIds = new Set([DOCUMENT_NODE_ID])
    this.mappedComponentsTree = null
    this.mappedDomNodeIds = new Map()
    this.nextMappedDomNodeId = COMPONENT_DOM_NODE_OFFSET
    this.cachedBoundsByRendererNodeId = new Map()
    this.unsubscribe = this.store.subscribe((mutation) => {
      this.handleMirrorMutation(mutation)
    })
  }

  dispose() {
    this.unsubscribe()
  }

  notifyInspectNode(rendererNodeId) {
    if (!this.domEnabled) {
      return
    }

    const backendNodeId = this.targetKind === "components"
      ? this.getMappedComponentsTree().rendererToDomNodeId.get(rendererNodeId) ?? DOCUMENT_NODE_ID
      : rendererNodeIdToDomNodeId(rendererNodeId)

    this.send({
      method: "Overlay.inspectNodeRequested",
      params: {
        backendNodeId,
      },
    })
  }

  async handleRequest(method, params) {
    switch (method) {
      case "Schema.getDomains": {
        return {
          domains: [...KNOWN_DOMAINS],
        }
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
        const root = this.targetKind === "components"
          ? componentNodeDescription(this.store, this.getMappedComponentsTree(), DOCUMENT_NODE_ID, depth)
          : nodeDescription(this.store, DOCUMENT_NODE_ID, depth)
        this.rememberKnownNode(root)
        return { root }
      }
      case "DOM.requestChildNodes": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID
        const depth = typeof params.depth === "number" ? params.depth : 1

        if (this.targetKind === "components") {
          const mappedTree = this.getMappedComponentsTree()
          const description = componentNodeDescription(this.store, mappedTree, nodeId, depth)
          const children = Array.isArray(description.children) ? description.children : []
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

        const description = nodeDescription(this.store, nodeId, depth)
        const children = Array.isArray(description.children) ? description.children : []
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
        const node = this.targetKind === "components"
          ? componentNodeDescription(this.store, this.getMappedComponentsTree(), nodeId, depth)
          : nodeDescription(this.store, nodeId, depth)
        this.rememberKnownNode(node)
        return { node }
      }
      case "DOM.getAttributes": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID
        if (nodeId === DOCUMENT_NODE_ID) {
          return { attributes: [] }
        }

        if (this.targetKind === "components") {
          const node = this.getMappedComponentsTree().nodes.get(nodeId)
          if (!node) {
            throw new Error(`Unknown DOM node ${nodeId}`)
          }

          return { attributes: componentAttributesForNode(node) }
        }

        const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
        const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
        if (!node) {
          throw new Error(`Unknown DOM node ${nodeId}`)
        }

        return { attributes: attributesForNode(node) }
      }
      case "DOM.getBoxModel": {
        return await this.handleGetBoxModel(params)
      }
      case "DOM.getNodeForLocation": {
        return await this.handleGetNodeForLocation(params)
      }
      case "DOM.resolveNode": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        if (this.targetKind === "components") {
          const node = this.getMappedComponentsTree().nodes.get(nodeId)
          if (!node) {
            throw new Error(`Unknown DOM node ${nodeId}`)
          }

          return { object: buildComponentRemoteObject(node) }
        }

        const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
        const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
        if (!node) {
          throw new Error(`Unknown DOM node ${nodeId}`)
        }

        return { object: buildNodeRemoteObject(node) }
      }
      case "DOM.requestNode": {
        if (this.targetKind === "components") {
          const nodeId = parseSyntheticComponentObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
          if (nodeId == null || !this.getMappedComponentsTree().nodes.has(nodeId)) {
            throw new Error("Unsupported object id")
          }

          this.knownDomNodeIds.add(nodeId)
          return { nodeId }
        }

        const rendererNodeId = parseSyntheticObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
        if (rendererNodeId == null) {
          throw new Error("Unsupported object id")
        }

        const nodeId = rendererNodeIdToDomNodeId(rendererNodeId)
        this.knownDomNodeIds.add(nodeId)
        return { nodeId }
      }
      case "DOM.pushNodesByBackendIdsToFrontend": {
        const backendNodeIds = Array.isArray(params.backendNodeIds)
          ? params.backendNodeIds.filter((value) => typeof value === "number")
          : []

        if (this.targetKind === "components") {
          const mappedTree = this.getMappedComponentsTree()
          return {
            nodeIds: backendNodeIds.map((backendNodeId) => {
              if (!mappedTree.nodes.has(backendNodeId)) {
                return 0
              }

              return this.materializeMappedNodePath(mappedTree, backendNodeId)
            }),
          }
        }

        return {
          nodeIds: backendNodeIds.map((backendNodeId) => {
            const rendererNodeId = domNodeIdToRendererNodeId(backendNodeId)
            if (rendererNodeId == null || !this.store.getNode(rendererNodeId)) {
              return 0
            }

            return this.materializeNodePath(rendererNodeId)
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

        if (this.targetKind === "components") {
          const node = this.getMappedComponentsTree().nodes.get(nodeId)
          if (!node) {
            throw new Error(`Unknown DOM node ${nodeId}`)
          }

          if (!node.source) {
            return {}
          }

          return {
            creation: {
              description: node.role === "component" ? "Qt Solid component" : "Qt Solid host node",
              callFrames: [
                {
                  functionName: componentNodeName(node),
                  scriptId: "",
                  url: sourceMetadataUrl(node.source),
                  lineNumber: Math.max(0, node.source.lineNumber - 1),
                  columnNumber: Math.max(0, node.source.columnNumber - 1),
                },
              ],
            },
          }
        }

        const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
        const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
        if (!node) {
          throw new Error(`Unknown DOM node ${nodeId}`)
        }

        const creation = creationStackTrace(node.owner, node.source, node.kind)
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

        if (this.targetKind === "components") {
          const mappedNode = this.getMappedComponentsTree().nodes.get(requestedNodeId)
          if (!mappedNode) {
            await this.safeNativeRequest("clearHighlight", {})
            return {}
          }

          await this.safeNativeRequest("highlightNode", { rendererNodeId: mappedNode.rendererNodeId })
          return {}
        }

        const rendererNodeId = domNodeIdToRendererNodeId(requestedNodeId)
        const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
        if (!node || node.kind === "qt-root") {
          await this.safeNativeRequest("clearHighlight", {})
          return {}
        }

        await this.safeNativeRequest("highlightNode", { rendererNodeId: node.id })
        return {}
      }
      case "Overlay.hideHighlight": {
        await this.safeNativeRequest("clearHighlight", {})
        return {}
      }
      case "CSS.getComputedStyleForNode": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        if (this.targetKind === "components") {
          return { computedStyle: [] }
        }

        const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
        const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
        if (!node) {
          return { computedStyle: [] }
        }

        return { computedStyle: styleEntriesForNode(node) }
      }
      case "CSS.getMatchedStylesForNode": {
        const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

        if (this.targetKind === "components") {
          return {
            inlineStyle: cssStyle([]),
            matchedCSSRules: [],
            pseudoElements: [],
            inherited: [],
            cssKeyframesRules: [],
          }
        }

        const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
        const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
        if (!node) {
          return {
            inlineStyle: cssStyle([]),
            matchedCSSRules: [],
            pseudoElements: [],
            inherited: [],
            cssKeyframesRules: [],
          }
        }

        return {
          inlineStyle: cssStyle(styleEntriesForNode(node)),
          matchedCSSRules: [],
          pseudoElements: [],
          inherited: [],
          cssKeyframesRules: [],
        }
      }
      case "Runtime.getProperties": {
        if (this.targetKind === "components") {
          const nodeId = parseSyntheticComponentObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
          if (nodeId == null) {
            throw new Error("Unsupported object id")
          }

          const node = this.getMappedComponentsTree().nodes.get(nodeId)
          if (!node) {
            throw new Error(`Unknown mapped component node ${nodeId}`)
          }

          return {
            result: componentNodeProperties(node),
            internalProperties: [],
          }
        }

        const rendererNodeId = parseSyntheticObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
        if (rendererNodeId == null) {
          throw new Error("Unsupported object id")
        }

        const node = this.store.getNode(rendererNodeId)
        if (!node) {
          throw new Error(`Unknown renderer node ${rendererNodeId}`)
        }

        return {
          result: nodeProperties(node),
          internalProperties: [],
        }
      }
      case "Runtime.releaseObject":
      case "Runtime.releaseObjectGroup": {
        if (this.targetKind === "components") {
          const nodeId = parseSyntheticComponentObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
          if (nodeId != null) {
            return {}
          }
        }

        const rendererNodeId = parseSyntheticObjectId(typeof params.objectId === "string" ? params.objectId : undefined)
        if (rendererNodeId != null) {
          return {}
        }

        throw new Error("Unsupported object id")
      }
      default:
        throw new Error(`Unsupported synthetic method ${method}`)
    }
  }

  async handleGetBoxModel(params) {
    const nodeId = typeof params.nodeId === "number" ? params.nodeId : DOCUMENT_NODE_ID

    if (this.targetKind === "components") {
      const mappedNode = this.getMappedComponentsTree().nodes.get(nodeId)
      if (!mappedNode) {
        throw new Error(`Unknown DOM node ${nodeId}`)
      }

      const bounds = await this.getBounds(mappedNode.rendererNodeId)
      return this.boxModelFromBounds(bounds)
    }

    const rendererNodeId = domNodeIdToRendererNodeId(nodeId)
    const node = rendererNodeId == null ? undefined : this.store.getNode(rendererNodeId)
    if (!node) {
      throw new Error(`Unknown DOM node ${nodeId}`)
    }

    if (node.kind === "qt-root") {
      return this.emptyBoxModel()
    }

    const bounds = await this.getBounds(node.id)
    return this.boxModelFromBounds(bounds)
  }

  async handleGetNodeForLocation(params) {
    const screenX = typeof params.x === "number" ? params.x : 0
    const screenY = typeof params.y === "number" ? params.y : 0
    const rendererNodeId = this.isPaused() ? null : await this.safeNativeRequest("getNodeAtPoint", { screenX, screenY }, null)

    if (this.targetKind === "components") {
      const mappedTree = this.getMappedComponentsTree()
      const domNodeId = rendererNodeId == null
        ? mappedTree.rootChildNodeIds[0] ?? DOCUMENT_NODE_ID
        : mappedTree.rendererToDomNodeId.get(rendererNodeId) ?? DOCUMENT_NODE_ID
      return {
        backendNodeId: domNodeId,
        nodeId: domNodeId,
        frameId: "qt-solid-components-frame",
      }
    }

    const snapshot = this.store.snapshot()
    const fallbackNodeId = snapshot.rootId == null ? DOCUMENT_NODE_ID : rendererNodeIdToDomNodeId(snapshot.rootId)
    const domNodeId = rendererNodeId == null ? fallbackNodeId : rendererNodeIdToDomNodeId(rendererNodeId)
    return {
      backendNodeId: domNodeId,
      nodeId: domNodeId,
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

  async getBounds(rendererNodeId) {
    if (this.isPaused()) {
      return this.cachedBoundsByRendererNodeId.get(rendererNodeId) ?? {
        visible: false,
        screenX: 0,
        screenY: 0,
        width: 0,
        height: 0,
      }
    }

    const bounds = await this.safeNativeRequest("getNodeBounds", { rendererNodeId }, null)
    if (bounds) {
      this.cachedBoundsByRendererNodeId.set(rendererNodeId, bounds)
      return bounds
    }

    return this.cachedBoundsByRendererNodeId.get(rendererNodeId) ?? {
      visible: false,
      screenX: 0,
      screenY: 0,
      width: 0,
      height: 0,
    }
  }

  async safeNativeRequest(method, params, fallback = {}) {
    if (this.isPaused()) {
      return fallback
    }

    try {
      return await this.nativeBridge.request(method, params, 150)
    } catch {
      return fallback
    }
  }

  allocateMappedDomNodeId(key) {
    const existing = this.mappedDomNodeIds.get(key)
    if (existing != null) {
      return existing
    }

    const domNodeId = this.nextMappedDomNodeId
    this.nextMappedDomNodeId += 1
    this.mappedDomNodeIds.set(key, domNodeId)
    return domNodeId
  }

  invalidateMappedComponentsTree() {
    this.mappedComponentsTree = null
  }

  appendMappedNode(mappedTree, node) {
    mappedTree.nodes.set(node.domNodeId, node)
    if (node.parentNodeId == null || node.parentNodeId === DOCUMENT_NODE_ID) {
      mappedTree.rootChildNodeIds.push(node.domNodeId)
      return
    }

    mappedTree.nodes.get(node.parentNodeId)?.childNodeIds.push(node.domNodeId)
  }

  extendMappedOwnerFrames(mappedTree, containerNodeId, activeFrames, nextFrames, rendererNodeId) {
    let commonPrefixLength = 0
    while (
      commonPrefixLength < activeFrames.length
      && commonPrefixLength < nextFrames.length
      && activeFrames[commonPrefixLength]?.frame.componentName === nextFrames[commonPrefixLength]?.componentName
      && formatSourceMetadata(activeFrames[commonPrefixLength]?.frame.source ?? { fileName: "", lineNumber: 0, columnNumber: 0 })
        === formatSourceMetadata(nextFrames[commonPrefixLength]?.source ?? { fileName: "", lineNumber: 0, columnNumber: 0 })
    ) {
      commonPrefixLength += 1
    }

    const currentFrames = activeFrames.slice(0, commonPrefixLength)
    let parentNodeId = currentFrames[currentFrames.length - 1]?.domNodeId ?? containerNodeId
    let componentPath = currentFrames.map((entry) => entry.frame.componentName).join(" > ")

    for (let index = commonPrefixLength; index < nextFrames.length; index += 1) {
      const frame = nextFrames[index]
      const domNodeId = this.allocateMappedDomNodeId([
        "component",
        String(containerNodeId),
        String(rendererNodeId),
        String(index),
        frame.componentName,
        frame.source ? formatSourceMetadata(frame.source) : "",
      ].join(":"))
      componentPath = componentPath ? `${componentPath} > ${frame.componentName}` : frame.componentName
      const rendererNode = this.store.getNode(rendererNodeId)
      const node = {
        domNodeId,
        parentNodeId,
        childNodeIds: [],
        rendererNodeId,
        rendererKind: rendererNode?.kind ?? "unknown",
        role: "component",
        componentName: frame.componentName,
        source: frame.source,
        componentPath,
      }
      this.appendMappedNode(mappedTree, node)
      currentFrames.push({ frame, domNodeId })
      parentNodeId = domNodeId
    }

    return currentFrames
  }

  mapRendererIntoComponents(mappedTree, rendererNodeId, containerNodeId, activeFrames) {
    const rendererNode = this.store.getNode(rendererNodeId)
    if (!rendererNode) {
      return
    }

    const nextFrames = this.extendMappedOwnerFrames(
      mappedTree,
      containerNodeId,
      activeFrames,
      rendererNode.owner?.ownerStack ?? [],
      rendererNodeId,
    )
    const anchorNodeId = nextFrames[nextFrames.length - 1]?.domNodeId ?? containerNodeId
    mappedTree.rendererToDomNodeId.set(rendererNodeId, anchorNodeId)

    for (const childId of rendererNode.childIds) {
      this.mapRendererIntoComponents(mappedTree, childId, containerNodeId, nextFrames)
    }
  }

  getMappedComponentsTree() {
    const snapshot = this.store.snapshot()
    if (this.mappedComponentsTree && this.mappedComponentsTree.revision === snapshot.revision) {
      return this.mappedComponentsTree
    }

    const mappedTree = {
      revision: snapshot.revision,
      nodes: new Map(),
      rendererToDomNodeId: new Map(),
      rootChildNodeIds: [],
    }

    if (snapshot.rootId != null) {
      const rootNode = this.store.getNode(snapshot.rootId)
      for (const childRendererNodeId of rootNode?.childIds ?? []) {
        const rendererNode = this.store.getNode(childRendererNodeId)
        if (!rendererNode) {
          continue
        }

        const domNodeId = this.allocateMappedDomNodeId(`host:${childRendererNodeId}`)
        this.appendMappedNode(mappedTree, {
          domNodeId,
          parentNodeId: DOCUMENT_NODE_ID,
          childNodeIds: [],
          rendererNodeId: childRendererNodeId,
          rendererKind: rendererNode.kind,
          role: "host",
          componentName: null,
          source: rendererNode.source,
          componentPath: rendererNode.kind,
        })
        mappedTree.rendererToDomNodeId.set(childRendererNodeId, domNodeId)
        this.mapRendererIntoComponents(mappedTree, childRendererNodeId, domNodeId, [])
      }
    }

    this.mappedComponentsTree = mappedTree
    return mappedTree
  }

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

  forgetKnownNodeSubtree(rendererNodeId) {
    this.knownDomNodeIds.delete(rendererNodeIdToDomNodeId(rendererNodeId))

    const node = this.store.getNode(rendererNodeId)
    if (!node) {
      return
    }

    for (const childId of node.childIds) {
      this.forgetKnownNodeSubtree(childId)
    }
  }

  isKnownDomNodeId(nodeId) {
    return this.knownDomNodeIds.has(nodeId)
  }

  sendChildNodeCountUpdated(parentRendererNodeId) {
    const parentDomNodeId = rendererNodeIdToDomNodeId(parentRendererNodeId)
    if (!this.isKnownDomNodeId(parentDomNodeId)) {
      return
    }

    const parent = this.store.getNode(parentRendererNodeId)
    if (!parent) {
      return
    }

    this.send({
      method: "DOM.childNodeCountUpdated",
      params: {
        nodeId: parentDomNodeId,
        childNodeCount: parent.childIds.length,
      },
    })
  }

  sendSetChildNodes(parentNodeId) {
    const nodes = childNodeDescriptions(this.store, parentNodeId)
    this.rememberKnownNodes(nodes)
    this.send({
      method: "DOM.setChildNodes",
      params: {
        parentId: parentNodeId,
        nodes,
      },
    })
  }

  materializeNodePath(rendererNodeId) {
    const path = []
    let currentId = rendererNodeId

    while (currentId != null) {
      path.push(currentId)
      currentId = this.store.getNode(currentId)?.parentId ?? null
    }

    path.reverse()

    let parentNodeId = DOCUMENT_NODE_ID
    for (const currentRendererNodeId of path) {
      const currentNodeId = rendererNodeIdToDomNodeId(currentRendererNodeId)
      if (!this.isKnownDomNodeId(currentNodeId)) {
        this.sendSetChildNodes(parentNodeId)
      }
      parentNodeId = currentNodeId
    }

    return rendererNodeIdToDomNodeId(rendererNodeId)
  }

  mappedChildNodeDescriptions(mappedTree, parentNodeId) {
    if (parentNodeId === DOCUMENT_NODE_ID) {
      return mappedTree.rootChildNodeIds.map((childId) => componentNodeDescription(this.store, mappedTree, childId, 0))
    }

    const node = mappedTree.nodes.get(parentNodeId)
    return node?.childNodeIds.map((childId) => componentNodeDescription(this.store, mappedTree, childId, 0)) ?? []
  }

  sendMappedSetChildNodes(mappedTree, parentNodeId) {
    const nodes = this.mappedChildNodeDescriptions(mappedTree, parentNodeId)
    this.rememberKnownNodes(nodes)
    this.send({
      method: "DOM.setChildNodes",
      params: {
        parentId: parentNodeId,
        nodes,
      },
    })
  }

  materializeMappedNodePath(mappedTree, domNodeId) {
    const path = []
    let currentNodeId = domNodeId

    while (currentNodeId != null && currentNodeId !== DOCUMENT_NODE_ID) {
      path.push(currentNodeId)
      currentNodeId = mappedTree.nodes.get(currentNodeId)?.parentNodeId ?? null
    }

    path.reverse()

    let parentNodeId = DOCUMENT_NODE_ID
    for (const currentDomNodeId of path) {
      if (!this.isKnownDomNodeId(currentDomNodeId)) {
        this.sendMappedSetChildNodes(mappedTree, parentNodeId)
      }
      parentNodeId = currentDomNodeId
    }

    return domNodeId
  }

  handleMirrorMutation(mutation) {
    if (!this.domEnabled) {
      return
    }

    if (this.targetKind === "components") {
      this.invalidateMappedComponentsTree()
      this.knownDomNodeIds.clear()
      this.knownDomNodeIds.add(DOCUMENT_NODE_ID)
      this.send({ method: "DOM.documentUpdated", params: {} })
      return
    }

    switch (mutation.type) {
      case "document-reset": {
        this.knownDomNodeIds.clear()
        this.knownDomNodeIds.add(DOCUMENT_NODE_ID)
        this.send({ method: "DOM.documentUpdated", params: {} })
        return
      }
      case "node-inserted": {
        const parentDomNodeId = rendererNodeIdToDomNodeId(mutation.parentId)
        if (!this.isKnownDomNodeId(parentDomNodeId)) {
          return
        }

        const nodeDomNodeId = rendererNodeIdToDomNodeId(mutation.nodeId)
        const node = nodeDescription(this.store, nodeDomNodeId, 0)
        this.rememberKnownNode(node)
        this.send({
          method: "DOM.childNodeInserted",
          params: {
            parentNodeId: parentDomNodeId,
            previousNodeId: mutation.previousSiblingId == null ? 0 : rendererNodeIdToDomNodeId(mutation.previousSiblingId),
            node,
          },
        })
        this.sendChildNodeCountUpdated(mutation.parentId)
        return
      }
      case "node-removed": {
        const parentDomNodeId = rendererNodeIdToDomNodeId(mutation.parentId)
        const nodeDomNodeId = rendererNodeIdToDomNodeId(mutation.nodeId)
        const shouldNotify = this.isKnownDomNodeId(parentDomNodeId) || this.isKnownDomNodeId(nodeDomNodeId)
        this.forgetKnownNodeSubtree(mutation.nodeId)
        if (!shouldNotify) {
          return
        }

        this.send({
          method: "DOM.childNodeRemoved",
          params: {
            parentNodeId: parentDomNodeId,
            nodeId: nodeDomNodeId,
          },
        })
        this.sendChildNodeCountUpdated(mutation.parentId)
        return
      }
      case "node-attribute-changed": {
        if (!isVisibleInlineAttributeName(mutation.name)) {
          return
        }

        const domNodeId = rendererNodeIdToDomNodeId(mutation.nodeId)
        if (!this.isKnownDomNodeId(domNodeId)) {
          return
        }

        const node = this.store.getNode(mutation.nodeId)
        if (!node) {
          return
        }

        const value = attributeValueForNode(node, mutation.name)
        if (value == null) {
          this.send({
            method: "DOM.attributeRemoved",
            params: {
              nodeId: domNodeId,
              name: mutation.name,
            },
          })
          return
        }

        this.send({
          method: "DOM.attributeModified",
          params: {
            nodeId: domNodeId,
            name: mutation.name,
            value,
          },
        })
        return
      }
      case "node-attribute-removed": {
        if (!isVisibleInlineAttributeName(mutation.name)) {
          return
        }

        const domNodeId = rendererNodeIdToDomNodeId(mutation.nodeId)
        if (!this.isKnownDomNodeId(domNodeId)) {
          return
        }

        this.send({
          method: "DOM.attributeRemoved",
          params: {
            nodeId: domNodeId,
            name: mutation.name,
          },
        })
        return
      }
      case "node-text-changed": {
        const domNodeId = rendererNodeIdToDomNodeId(mutation.nodeId)
        if (!this.isKnownDomNodeId(domNodeId)) {
          return
        }

        const node = this.store.getNode(mutation.nodeId)
        if (!node) {
          return
        }

        this.send({
          method: "DOM.characterDataModified",
          params: {
            nodeId: domNodeId,
            characterData: node.text ?? "",
          },
        })
      }
    }
  }

  send(payload) {
    this.sendPayload(payload)
  }
}

function isSyntheticRuntimeMethod(method, params) {
  const objectId = typeof params.objectId === "string" ? params.objectId : undefined
  if (method === "Runtime.getProperties" || method === "Runtime.releaseObject") {
    return objectId?.startsWith(SYNTHETIC_OBJECT_PREFIX) || objectId?.startsWith(SYNTHETIC_COMPONENT_OBJECT_PREFIX) || false
  }

  return method === "Runtime.releaseObjectGroup"
}

function isSyntheticMethod(method, params) {
  return method.startsWith("DOM.")
    || method.startsWith("CSS.")
    || method.startsWith("Overlay.")
    || isSyntheticRuntimeMethod(method, params)
}

class WorkerClientConnection {
  constructor(connectionId, targetKind, socket, store, nativeBridge) {
    this.connectionId = connectionId
    this.targetKind = targetKind
    this.socket = socket
    this.session = new inspector.Session()
    this.paused = false
    this.backend = new SyntheticBackend(
      targetKind,
      store,
      nativeBridge,
      (payload) => this.send(payload),
      () => this.paused,
    )
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

async function run() {
  const { port } = workerData
  const store = new MirrorInspectorStore()
  const nativeBridge = new NativeBridge()

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
      sendJson(response, [targetDescriptor(port, "renderer"), targetDescriptor(port, "components")])
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

    if (message?.type === "mirror-update") {
      store.replaceSnapshot(message.snapshot, message.mutation)
      return
    }

    if (message?.type === "inspect-node") {
      for (const connection of connections.values()) {
        connection.backend.notifyInspectNode(message.rendererNodeId)
      }
    }
  })

  websocketServer.on("connection", (socket, request) => {
    const targetKind = request.url === `/devtools/page/${COMPONENTS_TARGET_ID}` ? "components" : "renderer"
    const connectionId = `${nextConnectionId++}`
    const connection = new WorkerClientConnection(connectionId, targetKind, socket, store, nativeBridge)
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
    const validPaths = new Set([
      `/devtools/page/${SYNTHETIC_TARGET_ID}`,
      `/devtools/page/${COMPONENTS_TARGET_ID}`,
    ])
    if (!validPaths.has(request.url ?? "")) {
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
