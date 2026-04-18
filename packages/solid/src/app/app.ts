import type { QtApp, QtHostEvent } from "@qt-solid/core"
import type { JSX } from "solid-js"

import { mountQtScene, type AppWindowLifecycle } from "./mount.ts"
import type {
  AppDefinition,
  AppFactory,
  AppHandle,
  CreateAppOptions,
  RenderQtOptions,
  WindowHandle,
} from "./types.ts"

function isWindowHandle(input: AppDefinition): input is WindowHandle {
  return typeof input === "object" && input != null && "renderQt" in input && "dispose" in input
}

function normalizeAppDefinition(input: AppDefinition): CreateAppOptions {
  return isWindowHandle(input) ? { render: () => input.render() } : input
}

export function createApp(window: WindowHandle): AppHandle
export function createApp(options: CreateAppOptions): AppHandle
export function createApp(factory: AppFactory): AppHandle
export function createApp(input: AppDefinition | AppFactory): AppHandle {
  return {
    mount(app, options) {
      const definition = normalizeAppDefinition(typeof input === "function" ? input() : input)
      const mountedWindows = new Map<symbol, boolean>()
      let disposed = false
      let shuttingDown = false

      const quit = () => {
        if (disposed || shuttingDown) {
          return
        }

        shuttingDown = true
        app.shutdown()
      }

      const handleNativeEvent = (event: QtHostEvent) => {
        if (event.type === "app") {
          if (event.name === "activate") {
            definition.onActivate?.()
          }
          return
        }

        nativeEventTarget?.(event)
      }

      let nativeEventTarget: ((event: QtHostEvent) => void) | undefined

      const maybeHandleWindowAllClosed = () => {
        if (disposed || shuttingDown) {
          return
        }

        for (const open of mountedWindows.values()) {
          if (open) {
            return
          }
        }

        if (definition.onWindowAllClosed) {
          definition.onWindowAllClosed({ quit })
          return
        }

        quit()
      }

      const windowLifecycle: AppWindowLifecycle = {
        createCloseRequestedHandler(disposeWindow, userHandler) {
          if (userHandler) {
            return userHandler
          }

          return () => {
            disposeWindow()
          }
        },
        registerWindow(windowKey, open) {
          mountedWindows.set(windowKey, open)
        },
        setWindowOpen(windowKey, open) {
          if (!mountedWindows.has(windowKey)) {
            return
          }

          mountedWindows.set(windowKey, open)
          if (!open) {
            maybeHandleWindowAllClosed()
          }
        },
        unregisterWindow(windowKey) {
          mountedWindows.delete(windowKey)
          maybeHandleWindowAllClosed()
        },
      }

      const unmount = mountQtScene(
        definition.render,
        app,
        {
          attachNativeEvents(register) {
            nativeEventTarget = register
            options?.attachNativeEvents?.(handleNativeEvent)
          },
        },
        windowLifecycle,
        () => {
          disposed = true
          shuttingDown = true
        },
      )

      return {
        dispose() {
          if (disposed) {
            return
          }

          disposed = true
          unmount()
        },
      }
    },
  }
}

export function renderQt(node: () => JSX.Element, app: QtApp, options: RenderQtOptions = {}): () => void {
  return mountQtScene(node, app, options)
}
