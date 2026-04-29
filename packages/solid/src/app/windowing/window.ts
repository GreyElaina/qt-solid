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

import {
  createElement as createQtElement,
  insert as insertInto,
  setProp as setQtProp,
  spread as spreadQtProps,
  createCanvasFragmentBinding,
  destroyCanvasFragmentBinding,
  registerCanvasBinding,
  unregisterCanvasBinding,
  CanvasScopeContext,
} from "../../runtime/renderer.ts"

import { createApp } from "../app.ts"
import { QtAppWindowLifecycleContext, type AppWindowLifecycle } from "../mount.ts"
import { extendProps, toAccessor, windowPropsFrom } from "../props.ts"
import type { RenderQtOptions, WindowHandle } from "../types.ts"
import type { WindowComposable, WindowSource } from "./types.ts"

export function useWindow(source: WindowSource): WindowComposable {
  const read = toAccessor(source)

  return (children) => {
    const node = createQtElement("window")
    const windowNode = node as unknown as QtNode
    spreadQtProps(node, windowPropsFrom(read))

    const fragmentBinding = createCanvasFragmentBinding(windowNode.id)
    registerCanvasBinding(windowNode.id, fragmentBinding.root)

    onCleanup(() => {
      unregisterCanvasBinding(windowNode.id)
      destroyCanvasFragmentBinding(windowNode.id)
    })

    createComponent(CanvasScopeContext.Provider, {
      value: { canvasNodeId: windowNode.id, root: fragmentBinding.root },
      get children() {
        insertInto(fragmentBinding.root, children)
        return undefined
      },
    })

    return node
  }
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
      readFrameState: () => node.readWindowFrameState(),
      requestRepaint: () => node.requestRepaint(),
      requestNextFrame: () => node.requestNextFrame(),
      capture: () => node.captureWidget(),
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
    const windowNode = node as unknown as QtNode
    spreadQtProps(
      node,
      extendProps(windowPropsFrom(read), {
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

    const fragmentBinding = createCanvasFragmentBinding(windowNode.id)
    registerCanvasBinding(windowNode.id, fragmentBinding.root)

    onCleanup(() => {
      unregisterCanvasBinding(windowNode.id)
      destroyCanvasFragmentBinding(windowNode.id)
    })

    createComponent(CanvasScopeContext.Provider, {
      value: { canvasNodeId: windowNode.id, root: fragmentBinding.root },
      get children() {
        insertInto(fragmentBinding.root, body)
        return undefined
      },
    })

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

    return node
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
