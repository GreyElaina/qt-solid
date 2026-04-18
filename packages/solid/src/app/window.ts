import {
  createComponent,
  createEffect,
  createSignal,
  onCleanup,
  onMount,
  useContext,
  type Component,
  type JSX,
} from "solid-js"
import type { QtNode } from "@qt-solid/core/native"

import { createElement as createQtElement, setProp as setQtProp, spread as spreadQtProps } from "../runtime/reconciler.ts"

import { createApp } from "./app.ts"
import { QtAppWindowLifecycleContext, type AppWindowLifecycle } from "./mount.ts"
import { createRuntimeElement, extendProps, toAccessor, windowPropsFrom } from "./props.ts"
import type { RenderQtOptions, WindowComposable, WindowHandle, WindowSource } from "./types.ts"

export function useWindow(source: WindowSource): WindowComposable {
  const read = toAccessor(source)

  return (children) => createRuntimeElement("window", windowPropsFrom(read, children))
}

export function createWindow(source: WindowSource, body: () => JSX.Element): WindowHandle {
  const read = toAccessor(source)
  const [disposed, setDisposed] = createSignal(false)
  const windowKey = Symbol("qt-window-handle")
  let standaloneUnmount: (() => void) | undefined
  let activeWindowLifecycle: AppWindowLifecycle | undefined
  let currentNode: QtNode | undefined

  const requireNode = (): QtNode => {
    if (!currentNode) {
      throw new Error("Qt window node is not mounted")
    }

    return currentNode
  }

  const windowNative = () => {
    const node = requireNode()
    return {
      readFrameState: () => node.__qtSolidReadWindowFrameState(),
      requestRepaint: () => node.__qtSolidRequestRepaint(),
      requestNextFrame: () => node.__qtSolidRequestNextFrame(),
      capture: () => node.__qtSolidCaptureWidget(),
    }
  }

  const dispose = () => {
    setDisposed(true)
    activeWindowLifecycle?.setWindowOpen(windowKey, false)
    standaloneUnmount?.()
  }

  const open = () => {
    setDisposed(false)
    activeWindowLifecycle?.setWindowOpen(windowKey, true)
  }

  const WindowMount: Component = () => {
    const windowLifecycle = useContext(QtAppWindowLifecycleContext)

    onMount(() => {
      activeWindowLifecycle = windowLifecycle
      windowLifecycle?.registerWindow(windowKey, !disposed())
    })

    onCleanup(() => {
      if (activeWindowLifecycle === windowLifecycle) {
        activeWindowLifecycle = undefined
      }
      windowLifecycle?.unregisterWindow(windowKey)
    })

    const node = createQtElement("window")
    currentNode = node as unknown as QtNode
    spreadQtProps(
      node,
      extendProps(windowPropsFrom(read, body), {
        onCloseRequested: {
          enumerable: true,
          get() {
            const props = read()
            return windowLifecycle?.createCloseRequestedHandler(dispose, props.onCloseRequested)
              ?? props.onCloseRequested
          },
        },
      }),
    )

    onCleanup(() => {
      if (currentNode === (node as unknown as QtNode)) {
        currentNode = undefined
      }
    })

    let previousVisible = read().visible
    createEffect(() => {
      const nextVisible = read().visible ?? !disposed()
      setQtProp(node, "visible", nextVisible, previousVisible)
      previousVisible = nextVisible
    })

    return node as unknown as JSX.Element
  }

  const render = () => createComponent(WindowMount, {})
  const frameState = () => {
    const frame = windowNative().readFrameState()
    return {
      seq: frame.seq,
      elapsedMs: frame.elapsedMs,
      deltaMs: frame.deltaMs,
    }
  }
  const requestRepaint = () => {
    windowNative().requestRepaint()
  }
  const requestNextFrame = () => {
    windowNative().requestNextFrame()
  }
  const capture = () => windowNative().capture()
  return {
    render,
    renderQt(app, options: RenderQtOptions = {}) {
      const mounted = createApp({ render }).mount(app, options)
      const disposeStandalone = () => {
        if (standaloneUnmount !== disposeStandalone) {
          return
        }

        standaloneUnmount = undefined
        mounted.dispose()
      }

      standaloneUnmount = disposeStandalone
      return disposeStandalone
    },
    dispose,
    open,
    requestRepaint,
    requestNextFrame,
    frameState,
    capture,
  }
}
