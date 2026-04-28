import { QtApp, type QtHostEvent } from "@qt-solid/core"

import { startQtSolidDevtoolsServer, type QtSolidDevtoolsServer } from "../devtools/cdp-proxy.ts"
import { createQtSolidAppSession, type QtSolidAppSession } from "./bootstrap.ts"

interface QtSolidDevSessionState {
  app?: QtApp
  cleanupRegistered?: boolean
  devtoolsServerPromise?: Promise<QtSolidDevtoolsServer> | null
  pendingReplaceInput?: unknown
  replaceTimer?: ReturnType<typeof setTimeout>
  session?: QtSolidAppSession
}

interface QtSolidDevSessionSnapshot {
  app?: QtApp
  devtoolsServerPromise: Promise<QtSolidDevtoolsServer> | null
  session?: QtSolidAppSession
}

export interface QtSolidDevImportMeta {
  hot?: {
    accept(callback?: (module: { default?: unknown }) => void): void
  }
}

const QT_SOLID_DEV_SESSION_KEY = "__qtSolidDevSession__"
const QT_SOLID_DEV_REPLACE_KEY = "__qtSolidDevReplaceApp__"

function devSessionState(): QtSolidDevSessionState {
  const globalObject = globalThis as typeof globalThis & {
    [QT_SOLID_DEV_SESSION_KEY]?: QtSolidDevSessionState
  }

  globalObject[QT_SOLID_DEV_SESSION_KEY] ??= {}
  return globalObject[QT_SOLID_DEV_SESSION_KEY]
}

function installDevReplaceHook(): void {
  const replace = mountOrReplaceQtSolidDevApp
  const globalObject = globalThis as typeof globalThis & {
    [QT_SOLID_DEV_REPLACE_KEY]?: (input: unknown) => void
  }

  globalObject[QT_SOLID_DEV_REPLACE_KEY] = replace
}

function currentDevReplaceHook(): ((input: unknown) => void) | undefined {
  return (globalThis as typeof globalThis & {
    [QT_SOLID_DEV_REPLACE_KEY]?: (input: unknown) => void
  })[QT_SOLID_DEV_REPLACE_KEY]
}

function takeDevSessionState(): QtSolidDevSessionSnapshot {
  const state = devSessionState()
  const app = state.app
  const devtoolsServerPromise = state.devtoolsServerPromise ?? null
  const session = state.session

  if (state.replaceTimer) {
    clearTimeout(state.replaceTimer)
    state.replaceTimer = undefined
  }
  state.pendingReplaceInput = undefined
  state.app = undefined
  state.devtoolsServerPromise = null
  state.session = undefined

  return { app, devtoolsServerPromise, session }
}

function scheduleSessionReplace(input: unknown): void {
  const state = devSessionState()
  state.pendingReplaceInput = input

  if (state.replaceTimer) {
    return
  }

  state.replaceTimer = setTimeout(() => {
    state.replaceTimer = undefined
    const nextInput = state.pendingReplaceInput
    state.pendingReplaceInput = undefined
    if (!state.session || nextInput === undefined) {
      return
    }

    state.session.replace(nextInput)
  }, 0)
}

function disposeQtSolidDevAppSync(): void {
  const { app, devtoolsServerPromise, session } = takeDevSessionState()

  try {
    session?.dispose()
  } finally {
    app?.shutdown()
    if (devtoolsServerPromise) {
      void devtoolsServerPromise.then((server) => server.dispose())
    }
  }
}

function registerProcessCleanup(): void {
  const state = devSessionState()
  if (state.cleanupRegistered) {
    return
  }

  state.cleanupRegistered = true

  process.once("SIGINT", () => {
    void disposeQtSolidDevApp()
  })

  process.once("SIGTERM", () => {
    void disposeQtSolidDevApp()
  })

  process.once("exit", () => {
    disposeQtSolidDevAppSync()
  })
}

export function mountOrReplaceQtSolidDevApp(input: unknown): void {
  installDevReplaceHook()
  const state = devSessionState()

  if (state.session) {
    scheduleSessionReplace(input)
    return
  }

  const devtoolsEnabled = process.env.QT_SOLID_DEVTOOLS === "1"
  const devtoolsServerPromise = devtoolsEnabled ? startQtSolidDevtoolsServer() : null
  registerProcessCleanup()

  let handleHostEvent: (event: QtHostEvent) => void = () => {}
  const app = QtApp.start((event) => {
    handleHostEvent(event)
  })

  try {
    const session = createQtSolidAppSession(app, input, { devtoolsServerPromise })
    handleHostEvent = (event) => {
      session.handleHostEvent(event)
    }
    state.app = app
    state.devtoolsServerPromise = devtoolsServerPromise
    state.session = session
  } catch (error) {
    if (devtoolsServerPromise) {
      void devtoolsServerPromise.then((server) => server.dispose())
    }
    app.shutdown()
    throw error
  }
}

export function acceptQtSolidDevAppHmr(meta: QtSolidDevImportMeta, currentInput: unknown): void {
  meta.hot?.accept((module) => {
    currentDevReplaceHook()?.(module.default ?? currentInput)
  })
}

export async function disposeQtSolidDevApp(): Promise<void> {
  const { app, devtoolsServerPromise, session } = takeDevSessionState()

  try {
    session?.dispose()
  } finally {
    app?.shutdown()
    if (devtoolsServerPromise) {
      const server = await devtoolsServerPromise
      await server.dispose()
    }
  }
}
