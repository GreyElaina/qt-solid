import "../runtime/register-default-widgets.ts"

import { QtApp, type QtHostEvent } from "@qt-solid/core"
import { startQtSolidDevtoolsServer, type QtSolidDevtoolsServer } from "../devtools/cdp-proxy.ts"
import type { AppHandle, AppMount, AppMountOptions } from "../app/types.ts"

function isAppHandle(input: unknown): input is AppHandle {
  return typeof input === "object" && input != null && "mount" in input && typeof input.mount === "function"
}

function assertAppHandle(input: unknown): AppHandle {
  if (!isAppHandle(input)) {
    throw new TypeError("Qt Solid bootstrap entry default export must be createApp(...) result")
  }

  return input
}

export interface StartQtSolidAppResult {
  dispose(): void
}

export interface QtSolidAppSession {
  dispose(): void
  handleHostEvent(event: QtHostEvent): void
  replace(input: unknown): void
}

interface CreateQtSolidAppSessionOptions {
  devtoolsServerPromise?: Promise<QtSolidDevtoolsServer> | null
}

function mountAppHandle(
  app: QtApp,
  input: unknown,
  registerNativeEvents: (handler: (event: QtHostEvent) => void) => void,
): { handle: AppHandle, mount: AppMount } {
  const handle = assertAppHandle(input)
  const mount = handle.mount(app, {
    attachNativeEvents(handler) {
      registerNativeEvents(handler)
    },
  } satisfies AppMountOptions)

  return { handle, mount }
}

export function createQtSolidAppSession(
  app: QtApp,
  input: unknown,
  options: CreateQtSolidAppSessionOptions = {},
): QtSolidAppSession {
  const devtoolsServerPromise = options.devtoolsServerPromise ?? null
  let disposed = false
  let nativeEventTarget: (event: QtHostEvent) => void = () => {}

  const assignNativeEvents = (handler: (event: QtHostEvent) => void) => {
    nativeEventTarget = handler
  }

  let current = mountAppHandle(app, input, assignNativeEvents)

  return {
    dispose() {
      if (disposed) {
        return
      }

      disposed = true
      current.mount.dispose()
    },
    handleHostEvent(event) {
      if (devtoolsServerPromise && event.type === "inspect") {
        void devtoolsServerPromise.then((server) => {
          server.notifyInspectNode(event.nodeId)
        })
      }

      nativeEventTarget(event)
    },
    replace(nextInput) {
      if (disposed) {
        return
      }

      const previous = current
      previous.mount.dispose()

      try {
        current = mountAppHandle(app, nextInput, assignNativeEvents)
      } catch (error) {
        current = mountAppHandle(app, previous.handle, assignNativeEvents)
        throw error
      }
    },
  }
}

export function startQtSolidApp(input: unknown): StartQtSolidAppResult {
  const devtoolsEnabled = process.env.QT_SOLID_DEVTOOLS === "1"
  const devtoolsServerPromise = devtoolsEnabled ? startQtSolidDevtoolsServer() : null

  let session: QtSolidAppSession | undefined
  const app = QtApp.start((event) => {
    session?.handleHostEvent(event)
  })

  try {
    session = createQtSolidAppSession(app, input, { devtoolsServerPromise })
  } catch (error) {
    if (devtoolsServerPromise) {
      void devtoolsServerPromise.then((server) => server.dispose())
    }
    app.shutdown()
    throw error
  }

  let disposed = false

  const disposeMounted = (current: AppMount) => {
    if (disposed) {
      return
    }

    disposed = true

    try {
      current.dispose()
    } finally {
      app.shutdown()
      if (devtoolsServerPromise) {
        void devtoolsServerPromise.then((server) => server.dispose())
      }
    }
  }

  const exit = () => {
    if (session) {
      disposeMounted(session)
    }
    process.exit(0)
  }

  process.on("SIGINT", exit)
  process.on("SIGTERM", exit)

  return {
    dispose() {
      process.off("SIGINT", exit)
      process.off("SIGTERM", exit)
      if (session) {
        disposeMounted(session)
      }
    },
  }
}
