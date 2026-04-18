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

  it("serves synthetic DOM tree, forwards runtime evaluation, and streams DOM mutations", async () => {
    const projectRootUrl = pathToFileURL(`${process.cwd()}/`).href
    const devtoolsDemoFileUrl = pathToFileURL(resolve("examples/devtools-demo.tsx")).href

    rendererInspectorStore.reset(1)
    rendererInspectorStore.ensureElementNode(2, "window")
    rendererInspectorStore.setProp(2, "title", "devtools-demo")
    rendererInspectorStore.setSource(2, {
      fileName: "examples/devtools-demo.tsx",
      lineNumber: 12,
      columnNumber: 7,
      fileUrl: devtoolsDemoFileUrl,
      projectRootUrl,
    })
    rendererInspectorStore.setOwner(2, {
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
      ],
    })
    rendererInspectorStore.insertChild(1, 2)
    rendererInspectorStore.ensureElementNode(3, "view")
    rendererInspectorStore.setOwner(3, {
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
    rendererInspectorStore.insertChild(2, 3)
    rendererInspectorStore.ensureElementNode(4, "group")
    rendererInspectorStore.setOwner(4, {
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
    rendererInspectorStore.insertChild(3, 4)
    rendererInspectorStore.ensureElementNode(5, "input")
    rendererInspectorStore.setOwner(5, {
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
    rendererInspectorStore.setProp(5, "text", "hello")
    rendererInspectorStore.setProp(5, "placeholder", "0")
    rendererInspectorStore.insertChild(4, 5)

    const port = 9329 + Math.floor(Math.random() * 100)
    server = await startQtSolidDevtoolsServer(port)

    const targets = (await fetch(`http://127.0.0.1:${port}/json/list`).then((response) => response.json())) as Array<{
      id: string
      title: string
      webSocketDebuggerUrl: string
      type: string
    }>

    expect(targets).toHaveLength(2)
    const rendererTarget = targets.find((target) => target.id === "qt-solid-renderer")
    const componentsTarget = targets.find((target) => target.id === "qt-solid-components")
    expect(rendererTarget?.type).toBe("page")
    expect(componentsTarget?.type).toBe("page")
    expect(rendererTarget?.webSocketDebuggerUrl).toContain(`/devtools/page/qt-solid-renderer`)
    expect(componentsTarget?.webSocketDebuggerUrl).toContain(`/devtools/page/qt-solid-components`)

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

    const domains = (await call("Schema.getDomains")) as { domains: Array<{ name: string }> }
    expect(domains.domains.some((domain) => domain.name === "DOM")).toBe(true)
    expect(domains.domains.some((domain) => domain.name === "CSS")).toBe(true)

    await call("DOM.enable")
    const documentResult = (await call("DOM.getDocument", { depth: 3 })) as {
      root: {
        children?: Array<{
          nodeId: number
          localName?: string
          childNodeCount?: number
          children?: Array<{ nodeId: number; localName?: string; attributes?: string[] }>
        }>
      }
    }

    const rootNode = documentResult.root.children?.[0]
    const windowNode = rootNode?.children?.[0]
    expect(rootNode?.localName).toBe("qt-root")
    expect(rootNode?.childNodeCount).toBe(1)
    expect(windowNode?.localName).toBe("window")
    expect(windowNode?.attributes).toContain("title")
    expect(windowNode?.attributes).toContain("devtools-demo")
    expect(windowNode?.attributes).not.toContain("source")
    expect(windowNode?.attributes).not.toContain("owner-component")
    expect(windowNode?.attributes).not.toContain("owner-path")

    const componentsSocket = new WebSocket(componentsTarget!.webSocketDebuggerUrl)
    await new Promise<void>((resolve, reject) => {
      componentsSocket.once("open", () => resolve())
      componentsSocket.once("error", reject)
    })

    let componentsNextId = 1
    const componentsPending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>()
    componentsSocket.on("message", (raw) => {
      const message = JSON.parse(raw.toString()) as {
        id?: number
        result?: unknown
        error?: { message?: string }
      }

      if (message.id == null) {
        return
      }

      const current = componentsPending.get(message.id)
      if (!current) {
        return
      }

      componentsPending.delete(message.id)
      if (message.error) {
        current.reject(new Error(message.error.message ?? "cdp error"))
        return
      }

      current.resolve(message.result)
    })

    const callComponents = async (method: string, params?: Record<string, unknown>) => {
      const id = componentsNextId++
      const response = new Promise<unknown>((resolve, reject) => {
        componentsPending.set(id, { resolve, reject })
      })
      componentsSocket.send(JSON.stringify({ id, method, params }))
      return await response
    }

    await callComponents("DOM.enable")
    const componentsDocument = (await callComponents("DOM.getDocument", { depth: 6 })) as {
      root: {
        children?: Array<{
          nodeId: number
          localName?: string
          attributes?: string[]
          children?: Array<{
            nodeId: number
            localName?: string
            attributes?: string[]
            children?: Array<{
              nodeId: number
              localName?: string
              attributes?: string[]
              children?: Array<{
                nodeId: number
                localName?: string
                attributes?: string[]
                children?: Array<{
                  nodeId: number
                  localName?: string
                  attributes?: string[]
                  children?: Array<{ nodeId: number; localName?: string; attributes?: string[] }>
                }>
              }>
            }>
          }>
        }>
      }
    }
    const componentHostNode = componentsDocument.root.children?.[0]
    const demoComponentNode = componentHostNode?.children?.[0]
    const windowComponentNode = demoComponentNode?.children?.[0]
    const columnComponentNode = windowComponentNode?.children?.[0]
    const groupComponentNode = columnComponentNode?.children?.[0]
    const inputComponentNode = groupComponentNode?.children?.[0]
    expect(componentHostNode?.localName).toBe("window")
    expect(demoComponentNode?.localName).toBe("DevtoolsDemo")
    expect(windowComponentNode?.localName).toBe("Window")
    expect(columnComponentNode?.localName).toBe("Column")
    expect(groupComponentNode?.localName).toBe("Group")
    expect(inputComponentNode?.localName).toBe("Input")
    expect(inputComponentNode?.attributes).toContain("source")
    expect(inputComponentNode?.attributes).toContain("examples/devtools-demo.tsx:15:9")

    const resolvedInputComponent = (await callComponents("DOM.resolveNode", { nodeId: inputComponentNode?.nodeId })) as {
      object: { objectId: string }
    }
    const inputComponentProperties = (await callComponents("Runtime.getProperties", {
      objectId: resolvedInputComponent.object.objectId,
    })) as {
      result: Array<{ name: string; value?: { value?: unknown } }>
    }
    const componentPropertyValue = (name: string) => {
      const property = inputComponentProperties.result.find((entry) => entry.name === name)
      return property?.value?.value
    }
    expect(componentPropertyValue("componentName")).toBe("Input")
    expect(componentPropertyValue("componentPath")).toBe("DevtoolsDemo > Window > Column > Group > Input")
    expect(componentPropertyValue("rendererNodeId")).toBe(5)
    expect(componentPropertyValue("rendererKind")).toBe("input")
    expect(componentPropertyValue("frameKind")).toBe("user")

    const resolvedWindow = (await call("DOM.resolveNode", { nodeId: windowNode?.nodeId })) as {
      object: { objectId: string }
    }
    const originalCwd = process.cwd()
    const tempCwd = mkdtempSync(join(tmpdir(), "qt-solid-devtools-cwd-"))

    try {
      process.chdir(tempCwd)

      const windowProperties = (await call("Runtime.getProperties", {
        objectId: resolvedWindow.object.objectId,
      })) as {
        result: Array<{ name: string; value?: { value?: unknown } }>
      }
      const propertyValue = (name: string) => {
        const property = windowProperties.result.find((entry) => entry.name === name)
        return property?.value?.value
      }

      expect(windowProperties.result).toContainEqual(
        expect.objectContaining({ name: "ownerComponent", value: expect.objectContaining({ value: "Window" }) }),
      )
      expect(windowProperties.result).toContainEqual(
        expect.objectContaining({
          name: "ownerPath",
          value: expect.objectContaining({ value: "DevtoolsDemo > Window" }),
        }),
      )
      expect(propertyValue("sourceUrl")).toBe(devtoolsDemoFileUrl)
      expect(propertyValue("sourceFrameKind")).toBe("user")
      expect(propertyValue("sourceLocation")).toEqual({
        source: "examples/devtools-demo.tsx:12:7",
        sourceFileName: "examples/devtools-demo.tsx",
        sourceLineNumber: 12,
        sourceColumnNumber: 7,
        sourceUrl: devtoolsDemoFileUrl,
        frameKind: "user",
      })
      expect(propertyValue("ownerStack")).toEqual([
        {
          componentName: "DevtoolsDemo",
          source: "examples/devtools-demo.tsx:11:3",
          sourceFileName: "examples/devtools-demo.tsx",
          sourceLineNumber: 11,
          sourceColumnNumber: 3,
          sourceUrl: devtoolsDemoFileUrl,
          frameKind: "user",
        },
        {
          componentName: "Window",
          source: "examples/devtools-demo.tsx:12:7",
          sourceFileName: "examples/devtools-demo.tsx",
          sourceLineNumber: 12,
          sourceColumnNumber: 7,
          sourceUrl: devtoolsDemoFileUrl,
          frameKind: "user",
        },
      ])
      expect(propertyValue("creationLocation")).toEqual({
        functionName: "Window",
        scriptId: "",
        url: devtoolsDemoFileUrl,
        lineNumber: 11,
        columnNumber: 6,
        source: "examples/devtools-demo.tsx:12:7",
        sourceFileName: "examples/devtools-demo.tsx",
        sourceLineNumber: 12,
        sourceColumnNumber: 7,
        sourceUrl: devtoolsDemoFileUrl,
        frameKind: "user",
        frameRole: "owner",
      })
      expect(propertyValue("creationFrames")).toEqual([
        {
          functionName: "Window",
          scriptId: "",
          url: devtoolsDemoFileUrl,
          lineNumber: 11,
          columnNumber: 6,
          source: "examples/devtools-demo.tsx:12:7",
          sourceFileName: "examples/devtools-demo.tsx",
          sourceLineNumber: 12,
          sourceColumnNumber: 7,
          sourceUrl: devtoolsDemoFileUrl,
          frameKind: "user",
          frameRole: "owner",
        },
        {
          functionName: "DevtoolsDemo",
          scriptId: "",
          url: devtoolsDemoFileUrl,
          lineNumber: 10,
          columnNumber: 2,
          source: "examples/devtools-demo.tsx:11:3",
          sourceFileName: "examples/devtools-demo.tsx",
          sourceLineNumber: 11,
          sourceColumnNumber: 3,
          sourceUrl: devtoolsDemoFileUrl,
          frameKind: "user",
          frameRole: "owner",
        },
      ])

      await call("DOM.setNodeStackTracesEnabled", { enable: true })
      const creationStack = (await call("DOM.getNodeStackTraces", {
        nodeId: windowNode?.nodeId,
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
      expect(creationStack).toEqual({
        creation: {
          description: "Qt Solid node creation",
          callFrames: [
            {
              functionName: "Window",
              scriptId: "",
              url: devtoolsDemoFileUrl,
              lineNumber: 11,
              columnNumber: 6,
            },
            {
              functionName: "DevtoolsDemo",
              scriptId: "",
              url: devtoolsDemoFileUrl,
              lineNumber: 10,
              columnNumber: 2,
            },
          ],
        },
      })
    } finally {
      process.chdir(originalCwd)
      rmSync(tempCwd, { recursive: true, force: true })
    }

    const runtimeResult = (await call("Runtime.evaluate", { expression: "1 + 2" })) as {
      result: { value: number }
    }
    expect(runtimeResult.result.value).toBe(3)

    const debuggerEnableResult = (await call("Debugger.enable")) as { debuggerId?: string }
    expect(typeof debuggerEnableResult.debuggerId).toBe("string")

    const runtimeEnableResult = (await call("Runtime.enable")) as Record<string, unknown>
    expect(runtimeEnableResult).toEqual({})

    const highlightCalls: number[] = []
    const inspectModeCalls: boolean[] = []
    let clearHighlightCalls = 0
    const originalHighlightNode = qtSolidDebugPrimitives.highlightNode
    const originalGetNodeBounds = qtSolidDebugPrimitives.getNodeBounds
    const originalGetNodeAtPoint = qtSolidDebugPrimitives.getNodeAtPoint
    const originalSetInspectMode = qtSolidDebugPrimitives.setInspectMode
    const originalClearHighlight = qtSolidDebugPrimitives.clearHighlight
    qtSolidDebugPrimitives.highlightNode = (nodeId: number) => {
      highlightCalls.push(nodeId)
    }
    qtSolidDebugPrimitives.getNodeBounds = () => ({
      visible: true,
      screenX: 40,
      screenY: 60,
      width: 120,
      height: 32,
    })
    qtSolidDebugPrimitives.getNodeAtPoint = (screenX: number, screenY: number) => {
      expect(screenX).toBe(88)
      expect(screenY).toBe(72)
      return 5
    }
    qtSolidDebugPrimitives.setInspectMode = (enabled: boolean) => {
      inspectModeCalls.push(enabled)
    }
    qtSolidDebugPrimitives.clearHighlight = () => {
      clearHighlightCalls += 1
    }

    const boxModel = (await call("DOM.getBoxModel", { nodeId: windowNode?.nodeId })) as {
      model: { width: number; height: number; content: number[] }
    }
    expect(boxModel.model.width).toBe(120)
    expect(boxModel.model.height).toBe(32)
    expect(boxModel.model.content).toEqual([40, 60, 160, 60, 160, 92, 40, 92])

    const nodeForLocation = (await call("DOM.getNodeForLocation", { x: 88, y: 72 })) as {
      backendNodeId: number
      nodeId: number
      frameId: string
    }
    expect(nodeForLocation).toEqual({
      backendNodeId: 1005,
      nodeId: 1005,
      frameId: "qt-solid-frame",
    })

    await call("Overlay.setInspectMode", { mode: "searchForNode" })
    server.notifyInspectNode(5)
    const inspectRequested = await waitForNotification("Overlay.inspectNodeRequested")
    expect(inspectRequested.params).toEqual({ backendNodeId: 1005 })
    const pushedNodes = (await call("DOM.pushNodesByBackendIdsToFrontend", { backendNodeIds: [1005] })) as {
      nodeIds: number[]
    }
    expect(pushedNodes.nodeIds).toEqual([1005])
    const materializedParent = await waitForNotification(
      "DOM.setChildNodes",
      (notification) => (notification.params as { parentId?: number }).parentId === 1004,
    )
    expect(materializedParent.params).toMatchObject({
      parentId: 1004,
      nodes: [
        {
          nodeId: 1005,
          localName: "input",
          attributes: ["text", "hello", "placeholder", "0"],
        },
      ],
    })

    await call("Overlay.highlightNode", { nodeId: windowNode?.nodeId })
    await call("Overlay.hideHighlight")
    await call("Overlay.setInspectMode", { mode: "none" })
    expect(inspectModeCalls).toEqual([true, false])
    expect(highlightCalls).toEqual([2])
    expect(clearHighlightCalls).toBe(2)

    qtSolidDebugPrimitives.highlightNode = originalHighlightNode
    qtSolidDebugPrimitives.getNodeBounds = originalGetNodeBounds
    qtSolidDebugPrimitives.getNodeAtPoint = originalGetNodeAtPoint
    qtSolidDebugPrimitives.setInspectMode = originalSetInspectMode
    qtSolidDebugPrimitives.clearHighlight = originalClearHighlight

    rendererInspectorStore.setProp(2, "title", "mutated-title")
    const titleMutation = await waitForNotification(
      "DOM.attributeModified",
      (notification) => (notification.params as { name?: string }).name === "title",
    )
    expect(titleMutation.params).toMatchObject({
      nodeId: windowNode?.nodeId,
      name: "title",
      value: "mutated-title",
    })

    rendererInspectorStore.ensureTextNode(6, "literal hello")
    rendererInspectorStore.insertChild(2, 6)
    const inserted = await waitForNotification("DOM.childNodeInserted")
    expect(inserted.params).toMatchObject({
      parentNodeId: windowNode?.nodeId,
      previousNodeId: 1003,
      node: {
        nodeType: 3,
        nodeValue: "literal hello",
      },
    })

    const countAfterInsert = await waitForNotification("DOM.childNodeCountUpdated")
    expect(countAfterInsert.params).toMatchObject({
      nodeId: windowNode?.nodeId,
      childNodeCount: 2,
    })

    rendererInspectorStore.replaceText(6, "literal updated")
    const textMutation = await waitForNotification("DOM.characterDataModified")
    expect(textMutation.params).toMatchObject({
      nodeId: 1006,
      characterData: "literal updated",
    })

    rendererInspectorStore.removeChild(2, 6)
    const removed = await waitForNotification("DOM.childNodeRemoved")
    expect(removed.params).toMatchObject({
      parentNodeId: windowNode?.nodeId,
      nodeId: 1006,
    })

    const countAfterRemove = await waitForNotification("DOM.childNodeCountUpdated")
    expect(countAfterRemove.params).toMatchObject({
      nodeId: windowNode?.nodeId,
      childNodeCount: 1,
    })
    rendererInspectorStore.destroySubtree(6)

    expect(notifications.some((message) => message.method === "DOM.documentUpdated")).toBe(false)

    componentsSocket.close()
    await new Promise<void>((resolve) => {
      componentsSocket.once("close", () => resolve())
    })
    for (const [id, pendingEntry] of componentsPending) {
      componentsPending.delete(id)
      pendingEntry.reject(new Error("socket closed"))
    }

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
