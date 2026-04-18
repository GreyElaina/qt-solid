import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { createInterface } from "node:readline"
import { createServer } from "node:net"

import { afterEach, describe, expect } from "vitest"
import WebSocket from "ws"

import { nodeBin, projectRoot, testIfNativeSupported } from "./mocking/native-run"

interface CdpNotification {
  method: string
  params?: unknown
}

async function allocatePort(): Promise<number> {
  return await new Promise<number>((resolve, reject) => {
    const server = createServer()
    server.once("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      if (!address || typeof address === "string") {
        reject(new Error("failed to allocate port"))
        return
      }

      const { port } = address
      server.close((error) => {
        if (error) {
          reject(error)
          return
        }
        resolve(port)
      })
    })
  })
}

async function spawnBrokerHost(port: number): Promise<{ child: ChildProcessWithoutNullStreams; url: string }> {
  const child = spawn(nodeBin, ["./test/fixtures/devtools-broker-host.mjs"], {
    cwd: projectRoot,
    env: {
      ...process.env,
      QT_SOLID_DEVTOOLS_PORT: String(port),
    },
    stdio: ["pipe", "pipe", "pipe"],
  })

  const stdoutLines: string[] = []
  const stderrLines: string[] = []
  const stdout = createInterface({ input: child.stdout })
  const stderr = createInterface({ input: child.stderr })

  stdout.on("line", (line) => {
    stdoutLines.push(line)
  })
  stderr.on("line", (line) => {
    stderrLines.push(line)
  })

  return await new Promise<{ child: ChildProcessWithoutNullStreams; url: string }>((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill("SIGTERM")
      reject(new Error(`Timed out waiting for devtools broker ready\nstdout:\n${stdoutLines.join("\n")}\nstderr:\n${stderrLines.join("\n")}`))
    }, 5_000)

    const onExit = (code: number | null, signal: NodeJS.Signals | null) => {
      clearTimeout(timer)
      reject(new Error(`Broker child exited before ready (code=${String(code)} signal=${String(signal)})\nstdout:\n${stdoutLines.join("\n")}\nstderr:\n${stderrLines.join("\n")}`))
    }

    child.once("exit", onExit)

    stdout.on("line", (line) => {
      try {
        const message = JSON.parse(line) as { type?: string; url?: string }
        if (message.type !== "ready" || typeof message.url !== "string") {
          return
        }

        clearTimeout(timer)
        child.off("exit", onExit)
        resolve({ child, url: message.url })
      } catch {
        // ignore non-json lines
      }
    })
  })
}

async function terminateChild(child: ChildProcessWithoutNullStreams | null): Promise<void> {
  if (!child || child.killed) {
    return
  }

  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL")
    }, 2_000)

    child.once("exit", () => {
      clearTimeout(timer)
      resolve()
    })
    child.kill("SIGTERM")
  })
}

async function connectCdp(webSocketDebuggerUrl: string) {
  const socket = new WebSocket(webSocketDebuggerUrl)
  await new Promise<void>((resolve, reject) => {
    socket.once("open", () => resolve())
    socket.once("error", reject)
  })

  let nextId = 1
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>()
  const notifications: CdpNotification[] = []
  const waiters = new Set<{
    method: string
    predicate?: (notification: CdpNotification) => boolean
    resolve: (notification: CdpNotification) => void
    reject: (error: Error) => void
    timer: ReturnType<typeof setTimeout>
  }>()

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
      clearTimeout(current.timer)
      if (message.error) {
        current.reject(new Error(message.error.message ?? "cdp error"))
        return
      }

      current.resolve(message.result)
      return
    }

    if (!message.method) {
      return
    }

    const notification = { method: message.method, params: message.params }
    for (const waiter of waiters) {
      if (waiter.method !== notification.method) {
        continue
      }
      if (waiter.predicate && !waiter.predicate(notification)) {
        continue
      }

      waiters.delete(waiter)
      clearTimeout(waiter.timer)
      waiter.resolve(notification)
      return
    }

    notifications.push(notification)
  })

  const call = async (method: string, params?: Record<string, unknown>, timeoutMs = 2_000) => {
    const id = nextId++
    const response = new Promise<unknown>((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id)
        reject(new Error(`Timed out waiting for ${method}`))
      }, timeoutMs)
      pending.set(id, { resolve, reject, timer })
    })
    socket.send(JSON.stringify({ id, method, params }))
    return await response
  }

  const waitForNotification = async (
    method: string,
    predicate?: (notification: CdpNotification) => boolean,
    timeoutMs = 5_000,
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
        waiters.delete(waiter)
        reject(new Error(`Timed out waiting for ${method}`))
      }, timeoutMs)
      const waiter = { method, predicate, resolve, reject, timer }
      waiters.add(waiter)
    })
  }

  const close = async () => {
    for (const pendingEntry of pending.values()) {
      clearTimeout(pendingEntry.timer)
      pendingEntry.reject(new Error("socket closed"))
    }
    pending.clear()

    for (const waiter of waiters) {
      clearTimeout(waiter.timer)
      waiter.reject(new Error("socket closed"))
    }
    waiters.clear()

    socket.close()
    await new Promise<void>((resolve) => {
      socket.once("close", () => resolve())
    })
  }

  return { call, waitForNotification, notifications, close }
}

describe("qt solid devtools worker broker breakpoint", () => {
  let child: ChildProcessWithoutNullStreams | null = null

  afterEach(async () => {
    await terminateChild(child)
    child = null
  })

  testIfNativeSupported("holds Debugger.paused and still serves synthetic DOM from worker mirror", async () => {
    const port = await allocatePort()
    const spawned = await spawnBrokerHost(port)
    child = spawned.child

    const targets = (await fetch(`http://127.0.0.1:${port}/json/list`).then((response) => response.json())) as Array<{
      id: string
      webSocketDebuggerUrl: string
    }>
    const rendererTarget = targets.find((target) => target.id === "qt-solid-renderer")
    expect(rendererTarget?.webSocketDebuggerUrl).toBeDefined()

    const cdp = await connectCdp(rendererTarget!.webSocketDebuggerUrl)

    try {
      await cdp.call("DOM.enable")
      const documentResult = (await cdp.call("DOM.getDocument", { depth: 3 })) as {
        root: {
          children?: Array<{
            children?: Array<{
              nodeId: number
              localName?: string
              childNodeCount?: number
            }>
          }>
        }
      }
      const rootNode = documentResult.root.children?.[0]
      const windowNode = rootNode?.children?.[0]
      expect(windowNode?.localName).toBe("window")
      expect(windowNode?.childNodeCount).toBe(1)

      const resolvedWindow = (await cdp.call("DOM.resolveNode", { nodeId: windowNode?.nodeId })) as {
        object: { objectId: string }
      }

      await cdp.call("Debugger.enable")
      await cdp.call("Runtime.enable")
      await cdp.call("Runtime.evaluate", {
        expression: 'setTimeout(() => { globalThis.__qtSolidPauseProbe = "before"; debugger; globalThis.__qtSolidPauseProbe = "after" }, 0); "scheduled"',
      })

      const paused = await cdp.waitForNotification("Debugger.paused")
      expect((paused.params as { reason?: string } | undefined)?.reason).toBe("other")

      const pausedDocument = (await cdp.call("DOM.getDocument", { depth: 3 })) as {
        root: {
          children?: Array<{
            children?: Array<{
              localName?: string
            }>
          }>
        }
      }
      expect(pausedDocument.root.children?.[0]?.children?.[0]?.localName).toBe("window")

      const pausedProperties = (await cdp.call("Runtime.getProperties", {
        objectId: resolvedWindow.object.objectId,
      })) as {
        result: Array<{ name: string; value?: { value?: unknown } }>
      }
      expect(pausedProperties.result).toContainEqual(
        expect.objectContaining({
          name: "ownerPath",
          value: expect.objectContaining({ value: "DevtoolsDemo > Window" }),
        }),
      )

      const pausedStackTrace = (await cdp.call("DOM.getNodeStackTraces", {
        nodeId: windowNode?.nodeId,
      })) as {
        creation?: {
          callFrames?: Array<{ functionName?: string }>
        }
      }
      expect(pausedStackTrace.creation?.callFrames?.[0]?.functionName).toBe("Window")

      await new Promise((resolve) => setTimeout(resolve, 150))
      expect(cdp.notifications.some((message) => message.method === "Debugger.resumed")).toBe(false)

      await cdp.call("Debugger.resume")
      await cdp.waitForNotification("Debugger.resumed")

      const pauseProbe = (await cdp.call("Runtime.evaluate", {
        expression: "globalThis.__qtSolidPauseProbe",
      })) as {
        result?: { value?: string }
      }
      expect(pauseProbe.result?.value).toBe("after")
    } finally {
      await cdp.close()
    }
  })
})
