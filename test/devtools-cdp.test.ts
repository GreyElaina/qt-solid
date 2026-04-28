import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { pathToFileURL } from "node:url"

import { afterEach, describe, expect, it } from "vitest"
import WebSocket from "ws"

import { startQtSolidDevtoolsServer, type QtSolidDevtoolsServer } from "../packages/solid/src/devtools/cdp-proxy"
import { qtSolidDebugPrimitives } from "../packages/solid/src/devtools/debug-primitives"
import { rendererInspectorStore } from "../packages/solid/src/devtools/inspector-store"

interface CdpNotification {
  method: string
  params?: unknown
}

describe("qt solid devtools cdp proxy", () => {
  let server: QtSolidDevtoolsServer | null = null

  afterEach(async () => {
    if (server) {
      await server.dispose()
      server = null
    }
  })

  it("serves synthetic DOM tree from native fragment snapshots, forwards runtime evaluation, and streams DOM mutations", async () => {
    const projectRootUrl = pathToFileURL(`${process.cwd()}/`).href
    const devtoolsDemoFileUrl = pathToFileURL(resolve("examples/devtools-demo.tsx")).href
    const CANVAS_NODE_ID = 42

    // --- Set up metadata-only store ---
    rendererInspectorStore.addCanvas(CANVAS_NODE_ID)

    // Fragment 1: view (root-level fragment)
    rendererInspectorStore.setOwner(CANVAS_NODE_ID, 1, {
      ownerStack: [
        {
          componentName: "DevtoolsDemo",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 11,
            columnNumber: 3,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Window",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 12,
            columnNumber: 7,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Column",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 13,
            columnNumber: 5,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
      ],
    })
    rendererInspectorStore.setSource(CANVAS_NODE_ID, 1, {
      fileName: "examples/devtools-demo.tsx",
      lineNumber: 13,
      columnNumber: 5,
      fileUrl: devtoolsDemoFileUrl,
      projectRootUrl,
    })

    // Fragment 2: group (child of view)
    rendererInspectorStore.setOwner(CANVAS_NODE_ID, 2, {
      ownerStack: [
        {
          componentName: "DevtoolsDemo",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 11,
            columnNumber: 3,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Window",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 12,
            columnNumber: 7,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Column",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 13,
            columnNumber: 5,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Group",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 14,
            columnNumber: 7,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
      ],
    })

    // Fragment 3: input (child of group)
    rendererInspectorStore.setOwner(CANVAS_NODE_ID, 3, {
      ownerStack: [
        {
          componentName: "DevtoolsDemo",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 11,
            columnNumber: 3,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Window",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 12,
            columnNumber: 7,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Column",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 13,
            columnNumber: 5,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Group",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 14,
            columnNumber: 7,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
        {
          componentName: "Input",
          source: {
            fileName: "examples/devtools-demo.tsx",
            lineNumber: 15,
            columnNumber: 9,
            fileUrl: devtoolsDemoFileUrl,
            projectRootUrl,
          },
        },
      ],
    })

    // Emit creation/insertion events so the worker knows about the tree
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId: CANVAS_NODE_ID, fragmentId: 1, kind: "view" })
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId: CANVAS_NODE_ID, fragmentId: 2, kind: "group" })
    rendererInspectorStore.emit({ type: "node-created", canvasNodeId: CANVAS_NODE_ID, fragmentId: 3, kind: "input" })
    rendererInspectorStore.emit({
      type: "node-inserted",
      canvasNodeId: CANVAS_NODE_ID,
      parentFragmentId: null,
      childFragmentId: 1,
      anchorFragmentId: null,
    })
    rendererInspectorStore.emit({
      type: "node-inserted",
      canvasNodeId: CANVAS_NODE_ID,
      parentFragmentId: 1,
      childFragmentId: 2,
      anchorFragmentId: null,
    })
    rendererInspectorStore.emit({
      type: "node-inserted",
      canvasNodeId: CANVAS_NODE_ID,
      parentFragmentId: 2,
      childFragmentId: 3,
      anchorFragmentId: null,
    })

    // --- Set up native debug primitives mocks ---
    const originalFragmentTreeSnapshot = qtSolidDebugPrimitives.fragmentTreeSnapshot
    const originalHighlightFragment = qtSolidDebugPrimitives.highlightFragment
    const originalGetFragmentBounds = qtSolidDebugPrimitives.getFragmentBounds
    const originalFragmentHitTest = qtSolidDebugPrimitives.fragmentHitTest
    const originalSetInspectMode = qtSolidDebugPrimitives.setInspectMode
    const originalClearHighlight = qtSolidDebugPrimitives.clearHighlight
    const originalHighlightNode = qtSolidDebugPrimitives.highlightNode

    const nativeTree = [
      {
        id: 1,
        tag: "view",
        parentId: undefined,
        childIds: [2],
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        clip: false,
        visible: true,
        opacity: 1,
        props: {},
      },
      {
        id: 2,
        tag: "group",
        parentId: 1,
        childIds: [3],
        x: 10,
        y: 20,
        width: 200,
        height: 100,
        clip: false,
        visible: true,
        opacity: 1,
        props: {},
      },
      {
        id: 3,
        tag: "input",
        parentId: 2,
        childIds: [],
        x: 40,
        y: 60,
        width: 120,
        height: 32,
        clip: false,
        visible: true,
        opacity: 1,
        props: { text: "hello", placeholder: "0" },
      },
    ]

    qtSolidDebugPrimitives.fragmentTreeSnapshot = (_canvasNodeId: number) => {
      return nativeTree as any
    }

    const highlightFragmentCalls: Array<{ canvasNodeId: number; fragmentId: number | null }> = []
    qtSolidDebugPrimitives.highlightFragment = (canvasNodeId: number, fragmentId: number | null) => {
      highlightFragmentCalls.push({ canvasNodeId, fragmentId })
    }

    qtSolidDebugPrimitives.getFragmentBounds = (_canvasNodeId: number, _fragmentId: number) => {
      return {
        visible: true,
        screenX: 40,
        screenY: 60,
        width: 120,
        height: 32,
      }
    }

    qtSolidDebugPrimitives.fragmentHitTest = (_canvasNodeId: number, x: number, y: number) => {
      expect(x).toBe(88)
      expect(y).toBe(72)
      return 3 // input fragment
    }

    const inspectModeCalls: boolean[] = []
    qtSolidDebugPrimitives.setInspectMode = (enabled: boolean) => {
      inspectModeCalls.push(enabled)
    }

    let clearHighlightCalls = 0
    qtSolidDebugPrimitives.clearHighlight = () => {
      clearHighlightCalls += 1
    }

    const port = 9329 + Math.floor(Math.random() * 100)
    server = await startQtSolidDevtoolsServer(port)

    const targets = (await fetch(`http://127.0.0.1:${port}/json/list`).then((response) => response.json())) as Array<{
      id: string
      title: string
      webSocketDebuggerUrl: string
      type: string
    }>

    expect(targets).toHaveLength(1)
    const rendererTarget = targets.find((target) => target.id === "qt-solid-renderer")
    expect(rendererTarget?.type).toBe("page")
    expect(rendererTarget?.webSocketDebuggerUrl).toContain(`/devtools/page/qt-solid-renderer`)

    const socket = new WebSocket(rendererTarget!.webSocketDebuggerUrl)
    await new Promise<void>((resolve, reject) => {
      socket.once("open", () => resolve())
      socket.once("error", reject)
    })

    let nextId = 1
    const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>()
    const notifications: CdpNotification[] = []
    const notificationWaiters = new Set<{
      method: string
      predicate?: (notification: CdpNotification) => boolean
      resolve: (notification: CdpNotification) => void
      reject: (error: Error) => void
      timer: ReturnType<typeof setTimeout>
    }>()

    const maybeResolveNotification = (notification: CdpNotification): boolean => {
      for (const waiter of notificationWaiters) {
        if (waiter.method !== notification.method) {
          continue
        }

        if (waiter.predicate && !waiter.predicate(notification)) {
          continue
        }

        clearTimeout(waiter.timer)
        notificationWaiters.delete(waiter)
        waiter.resolve(notification)
        return true
      }

      return false
    }

    const waitForNotification = async (
      method: string,
      predicate?: (notification: CdpNotification) => boolean,
    ): Promise<CdpNotification> => {
      for (let index = 0; index < notifications.length; index += 1) {
        const notification = notifications[index]!
        if (notification.method !== method) {
          continue
        }

        if (predicate && !predicate(notification)) {
          continue
        }

        notifications.splice(index, 1)
        return notification
      }

      return await new Promise<CdpNotification>((resolve, reject) => {
        const timer = setTimeout(() => {
          notificationWaiters.delete(waiter)
          reject(new Error(`Timed out waiting for ${method}`))
        }, 2_000)

        const waiter = {
          method,
          predicate,
          resolve,
          reject,
          timer,
        }
        notificationWaiters.add(waiter)
      })
    }

    socket.on("message", (raw) => {
      const message = JSON.parse(raw.toString()) as {
        id?: number
        result?: unknown
        error?: { message?: string }
        method?: string
        params?: unknown
      }

      if (message.id != null) {
        const current = pending.get(message.id)
        if (!current) {
          return
        }

        pending.delete(message.id)
        if (message.error) {
          current.reject(new Error(message.error.message ?? "cdp error"))
          return
        }

        current.resolve(message.result)
        return
      }

      if (message.method) {
        const notification = { method: message.method, params: message.params }
        if (!maybeResolveNotification(notification)) {
          notifications.push(notification)
        }
      }
    })

    const call = async (method: string, params?: Record<string, unknown>) => {
      const id = nextId++
      const response = new Promise<unknown>((resolve, reject) => {
        pending.set(id, { resolve, reject })
      })
      socket.send(JSON.stringify({ id, method, params }))
      return await response
    }

    // --- Schema ---
    const domains = (await call("Schema.getDomains")) as { domains: Array<{ name: string }> }
    expect(domains.domains.some((domain) => domain.name === "DOM")).toBe(true)
    expect(domains.domains.some((domain) => domain.name === "CSS")).toBe(true)

    // --- DOM tree ---
    await call("DOM.enable")
    const documentResult = (await call("DOM.getDocument", { depth: 4 })) as {
      root: {
        childNodeCount?: number
        children?: Array<{
          nodeId: number
          localName?: string
          childNodeCount?: number
          children?: Array<{
            nodeId: number
            localName?: string
            attributes?: string[]
            childNodeCount?: number
            children?: Array<{
              nodeId: number
              localName?: string
              attributes?: string[]
              childNodeCount?: number
              children?: Array<{ nodeId: number; localName?: string; attributes?: string[] }>
            }>
          }>
        }>
      }
    }

    // New structure: #document → [WINDOW per canvas] → [fragment nodes]
    expect(documentResult.root.childNodeCount).toBe(1)
    const windowNode = documentResult.root.children?.[0]
    expect(windowNode?.localName).toBe("window")
    expect(windowNode?.childNodeCount).toBe(1) // one root fragment (view)

    const viewNode = windowNode?.children?.[0]
    expect(viewNode?.localName).toBe("view")
    expect(viewNode?.childNodeCount).toBe(1) // one child (group)

    const groupNode = viewNode?.children?.[0]
    expect(groupNode?.localName).toBe("group")
    expect(groupNode?.childNodeCount).toBe(1)

    const inputNode = groupNode?.children?.[0]
    expect(inputNode?.localName).toBe("input")
    expect(inputNode?.attributes).toContain("text")
    expect(inputNode?.attributes).toContain("hello")

    // --- DOM.resolveNode + Runtime.getProperties ---
    // Resolve the view node to get its object ID
    const resolvedView = (await call("DOM.resolveNode", { nodeId: viewNode?.nodeId })) as {
      object: { objectId: string }
    }
    expect(resolvedView.object.objectId).toContain("qt-solid-frag:")

    const originalCwd = process.cwd()
    const tempCwd = mkdtempSync(join(tmpdir(), "qt-solid-devtools-cwd-"))

    try {
      process.chdir(tempCwd)

      const viewProperties = (await call("Runtime.getProperties", {
        objectId: resolvedView.object.objectId,
      })) as {
        result: Array<{ name: string; value?: { value?: unknown } }>
      }
      const propertyValue = (name: string) => {
        const property = viewProperties.result.find((entry) => entry.name === name)
        return property?.value?.value
      }

      expect(propertyValue("kind")).toBe("view")
      expect(viewProperties.result).toContainEqual(
        expect.objectContaining({ name: "ownerComponent", value: expect.objectContaining({ value: "Column" }) }),
      )
      expect(viewProperties.result).toContainEqual(
        expect.objectContaining({
          name: "ownerPath",
          value: expect.objectContaining({ value: "DevtoolsDemo > Window > Column" }),
        }),
      )
      expect(propertyValue("sourceUrl")).toBe(devtoolsDemoFileUrl)
      expect(propertyValue("sourceFrameKind")).toBe("user")
    } finally {
      process.chdir(originalCwd)
      rmSync(tempCwd, { recursive: true, force: true })
    }

    // --- getNodeStackTraces ---
    await call("DOM.setNodeStackTracesEnabled", { enable: true })
    const creationStack = (await call("DOM.getNodeStackTraces", {
      nodeId: viewNode?.nodeId,
    })) as {
      creation?: {
        description?: string
        callFrames?: Array<{
          functionName?: string
          scriptId?: string
          url?: string
          lineNumber?: number
          columnNumber?: number
        }>
      }
    }
    expect(creationStack.creation).toBeDefined()
    expect(creationStack.creation?.description).toBe("Qt Solid node creation")
    expect(creationStack.creation?.callFrames).toBeDefined()
    expect((creationStack.creation?.callFrames?.length ?? 0)).toBeGreaterThan(0)

    // --- Runtime.evaluate (forwarded to real inspector) ---
    const runtimeResult = (await call("Runtime.evaluate", { expression: "1 + 2" })) as {
      result: { value: number }
    }
    expect(runtimeResult.result.value).toBe(3)

    // --- Debugger.enable (forwarded) ---
    const debuggerEnableResult = (await call("Debugger.enable")) as { debuggerId?: string }
    expect(typeof debuggerEnableResult.debuggerId).toBe("string")

    // --- Runtime.enable (forwarded) ---
    const runtimeEnableResult = (await call("Runtime.enable")) as Record<string, unknown>
    expect(runtimeEnableResult).toEqual({})

    // --- Box model ---
    const inputNodeId = inputNode?.nodeId
    expect(inputNodeId).toBeDefined()

    const boxModel = (await call("DOM.getBoxModel", { nodeId: inputNodeId })) as {
      model: { width: number; height: number; content: number[] }
    }
    expect(boxModel.model.width).toBe(120)
    expect(boxModel.model.height).toBe(32)
    expect(boxModel.model.content).toEqual([40, 60, 160, 60, 160, 92, 40, 92])

    // --- Hit test ---
    const nodeForLocation = (await call("DOM.getNodeForLocation", { x: 88, y: 72 })) as {
      backendNodeId: number
      nodeId: number
      frameId: string
    }
    // Should return the input fragment's allocated domNodeId
    expect(nodeForLocation.frameId).toBe("qt-solid-frame")
    expect(nodeForLocation.backendNodeId).toBe(nodeForLocation.nodeId)
    // The returned nodeId should be the input fragment's ID
    expect(nodeForLocation.nodeId).toBe(inputNodeId)

    // --- Overlay ---
    await call("Overlay.setInspectMode", { mode: "searchForNode" })
    expect(inspectModeCalls).toEqual([true])

    // Highlight the input node
    await call("Overlay.highlightNode", { nodeId: inputNodeId })
    expect(highlightFragmentCalls.length).toBeGreaterThan(0)
    const lastHighlight = highlightFragmentCalls[highlightFragmentCalls.length - 1]
    expect(lastHighlight?.canvasNodeId).toBe(CANVAS_NODE_ID)
    expect(lastHighlight?.fragmentId).toBe(3) // input fragment

    await call("Overlay.hideHighlight")
    await call("Overlay.setInspectMode", { mode: "none" })
    expect(inspectModeCalls).toEqual([true, false])

    // --- pushNodesByBackendIdsToFrontend ---
    const pushedNodes = (await call("DOM.pushNodesByBackendIdsToFrontend", { backendNodeIds: [inputNodeId] })) as {
      nodeIds: number[]
    }
    expect(pushedNodes.nodeIds).toEqual([inputNodeId])

    // --- Structural mutation: emit a text-changed event ---
    rendererInspectorStore.emit({
      type: "text-changed",
      canvasNodeId: CANVAS_NODE_ID,
      fragmentId: 3,
      value: "updated text",
    })

    // The worker should send DOM.documentUpdated or DOM.characterDataModified
    // Since text-changed sends characterDataModified for known nodes, wait for it
    // Actually, text-changed sends characterDataModified only if node is known; otherwise it's benign
    // The inputNodeId was made known via getDocument depth:3

    // Wait a tick for the event to propagate
    await new Promise((resolve) => setTimeout(resolve, 100))

    // --- Structural mutation: emit node-inserted → should trigger DOM.documentUpdated ---
    rendererInspectorStore.emit({
      type: "node-created",
      canvasNodeId: CANVAS_NODE_ID,
      fragmentId: 4,
      kind: "Text",
    })
    rendererInspectorStore.emit({
      type: "node-inserted",
      canvasNodeId: CANVAS_NODE_ID,
      parentFragmentId: 1,
      childFragmentId: 4,
      anchorFragmentId: null,
    })

    // Update native tree snapshot to include the new node
    nativeTree.push({
      id: 4,
      tag: "Text",
      parentId: 1,
      childIds: [],
      x: 0,
      y: 0,
      width: 100,
      height: 20,
      clip: false,
      visible: true,
      opacity: 1,
      props: { text: "new text node" } as Record<string, string>,
    })
    // Update view's childIds
    nativeTree[0]!.childIds = [2, 4]

    const documentUpdated = await waitForNotification("DOM.documentUpdated")
    expect(documentUpdated).toBeDefined()

    // After DOM.documentUpdated, DevTools would re-fetch
    const documentResult2 = (await call("DOM.getDocument", { depth: 4 })) as {
      root: {
        children?: Array<{
          children?: Array<{
            childNodeCount?: number
            children?: Array<{ localName?: string; nodeType?: number; nodeValue?: string }>
          }>
        }>
      }
    }
    const viewNode2 = documentResult2.root.children?.[0]?.children?.[0]
    expect(viewNode2?.childNodeCount).toBe(2) // group + text

    // --- Cleanup debug primitive mocks ---
    qtSolidDebugPrimitives.fragmentTreeSnapshot = originalFragmentTreeSnapshot
    qtSolidDebugPrimitives.highlightFragment = originalHighlightFragment
    qtSolidDebugPrimitives.getFragmentBounds = originalGetFragmentBounds
    qtSolidDebugPrimitives.fragmentHitTest = originalFragmentHitTest
    qtSolidDebugPrimitives.setInspectMode = originalSetInspectMode
    qtSolidDebugPrimitives.clearHighlight = originalClearHighlight
    qtSolidDebugPrimitives.highlightNode = originalHighlightNode

    // Clean up canvas
    rendererInspectorStore.removeCanvas(CANVAS_NODE_ID)

    socket.close()
    await new Promise<void>((resolve) => {
      socket.once("close", () => resolve())
    })

    for (const waiter of notificationWaiters) {
      clearTimeout(waiter.timer)
      notificationWaiters.delete(waiter)
      waiter.reject(new Error("socket closed"))
    }
  })
})
