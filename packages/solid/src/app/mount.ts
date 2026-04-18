import { createComponent, createContext, createRoot, type JSX } from "solid-js"
import type { QtApp } from "@qt-solid/core"

import { createNativeRendererBinding, type NativeQtRendererBinding } from "../runtime/native-renderer-binding.ts"
import { _render as renderQtTree, QtRendererBindingContext, rememberQtRendererNode } from "../runtime/reconciler.ts"
import type { QtRendererNode } from "../runtime/renderer.ts"

import type { RenderQtOptions } from "./types.ts"

export interface AppWindowLifecycle {
  createCloseRequestedHandler(disposeWindow: () => void, userHandler?: () => void): (() => void) | undefined
  registerWindow(windowKey: symbol, open: boolean): void
  setWindowOpen(windowKey: symbol, open: boolean): void
  unregisterWindow(windowKey: symbol): void
}

const ACTIVE_QT_MOUNTS = new WeakMap<QtApp, { dispose: () => void }>()

export const QtAppWindowLifecycleContext = createContext<AppWindowLifecycle | undefined>(undefined)

function destroyRootChildren(root: QtRendererNode): void {
  let child = root.firstChild
  while (child) {
    const next = child.nextSibling
    child.destroy()
    child = next
  }
}

function mountQtRoot(
  binding: NativeQtRendererBinding,
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
      const renderDispose = renderQtTree(
        () =>
          createComponent(QtRendererBindingContext.Provider, {
            value: binding,
            get children() {
              if (!windowLifecycle) {
                return node()
              }

              return createComponent(QtAppWindowLifecycleContext.Provider, {
                value: windowLifecycle,
                get children() {
                  return node()
                },
              })
            },
          }),
        binding.root,
      )

      return () => {
        renderDispose()
        destroyRootChildren(binding.root)
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
  const binding = createNativeRendererBinding(app)
  rememberQtRendererNode(binding.root, binding)
  options.attachNativeEvents?.((event) => {
    binding.handleEvent(event)
  })
  return mountQtRoot(binding, node, app, windowLifecycle, onShutdown)
}
