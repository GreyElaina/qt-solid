import { createComponent, createContext, createRoot, type JSX } from "solid-js"
import type { QtApp } from "@qt-solid/core"

import { initRenderer, handleEvent, nativeRoot, destroyChildWidgets } from "../runtime/renderer.ts"

import type { RenderQtOptions } from "./types.ts"

export interface AppWindowLifecycle {
  createCloseRequestedHandler(disposeWindow: () => void, userHandler?: () => void): (() => void) | undefined
  registerWindow(windowKey: symbol, open: boolean): void
  setWindowOpen(windowKey: symbol, open: boolean): void
  unregisterWindow(windowKey: symbol): void
}

const ACTIVE_QT_MOUNTS = new WeakMap<QtApp, { dispose: () => void }>()

export const QtAppWindowLifecycleContext = createContext<AppWindowLifecycle | undefined>(undefined)

function mountQtRoot(
  node: () => JSX.Element,
  app?: QtApp,
  windowLifecycle?: AppWindowLifecycle,
  onShutdown?: () => void,
): () => void {
  let disposeRoot: (() => void) | undefined
  let disposeRequested = false
  let disposed = false
  let cleanupRegistration: (() => void) | undefined
  let shutdownRequested = false
  let shutdownOriginal: (() => void) | undefined
  let mounting = app != null

  const root = nativeRoot()

  const finishDispose = () => {
    if (disposed) {
      return
    }

    if (!disposeRoot) {
      disposeRequested = true
      return
    }

    disposed = true
    cleanupRegistration?.()
    cleanupRegistration = undefined
    disposeRoot()
  }

  if (app) {
    const existing = ACTIVE_QT_MOUNTS.get(app)
    if (existing) {
      throw new Error("Qt Solid root is already mounted for this app")
    }

    shutdownOriginal = app.shutdown.bind(app)
    ;(app as QtApp & { shutdown: () => void }).shutdown = (() => {
      onShutdown?.()

      if (mounting) {
        shutdownRequested = true
        return
      }

      finishDispose()
      shutdownOriginal?.()
    }) as typeof app.shutdown

    cleanupRegistration = () => {
      const current = ACTIVE_QT_MOUNTS.get(app)
      if (current?.dispose === finishDispose) {
        ACTIVE_QT_MOUNTS.delete(app)
      }

      if (shutdownOriginal) {
        ;(app as QtApp & { shutdown: () => void }).shutdown = shutdownOriginal as typeof app.shutdown
      }
    }

    ACTIVE_QT_MOUNTS.set(app, { dispose: finishDispose })
  }

  try {
    disposeRoot = createRoot((dispose) => {
      // Window components are portals — they insert themselves into root.
      // We just need to execute the user's render function within a reactive root.
      if (!windowLifecycle) {
        node()
      } else {
        createComponent(QtAppWindowLifecycleContext.Provider, {
          value: windowLifecycle,
          get children() {
            return node()
          },
        })
      }

      return () => {
        destroyChildWidgets(root)
        dispose()
      }
    })
  } catch (error) {
    cleanupRegistration?.()
    cleanupRegistration = undefined
    throw error
  } finally {
    mounting = false
  }

  if (disposeRequested) {
    finishDispose()
  }

  if (shutdownRequested) {
    finishDispose()
    shutdownOriginal?.()
  }

  return finishDispose
}

export function mountQtScene(
  node: () => JSX.Element,
  app: QtApp,
  options: RenderQtOptions = {},
  windowLifecycle?: AppWindowLifecycle,
  onShutdown?: () => void,
): () => void {
  initRenderer(app)
  options.attachNativeEvents?.((event) => {
    handleEvent(event)
  })
  return mountQtRoot(node, app, windowLifecycle, onShutdown)
}
