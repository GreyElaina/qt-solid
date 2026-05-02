import {
  children as resolveChildren,
  createComponent,
  createContext,
  createEffect,
  createSignal,
  onCleanup,
  useContext,
  type Accessor,
  type Component,
  type JSX,
} from "solid-js"
import type { QtNode } from "@qt-solid/core/native"
import {
  focusWidget,
  getNodeBounds,
  getScreenGeometry,
  getWidgetSizeHint,
  setWindowTransientOwner,
  canvasComputeIntrinsicSize,
} from "@qt-solid/core/native"

import {
  createElement as createQtElement,
  insert as insertInto,
  insertNode as insertQtNode,
  setProp as setQtProp,
  spread as spreadQtProps,
  createCanvasFragmentBinding,
  destroyCanvasFragmentBinding,
  registerCanvasBinding,
  unregisterCanvasBinding,
  CanvasScopeContext,
  nativeRoot,
} from "../../runtime/renderer.ts"

import { extendProps, getter, toAccessor, windowPropsFrom } from "../props.ts"
import type { PopupDismissEvent, PopupProps, PopupSource, WindowProps } from "./types.ts"

interface PopupOwnerState {
  readonly id: number
  dismiss(): void
}

const PopupOwnerContext = createContext<PopupOwnerState | null>(null)

function computePopupPosition(
  props: PopupProps,
  popupNodeId: number,
  measuredSize?: { width: number; height: number },
): { x: number; y: number } | undefined {
  const anchor = props.anchor
  if (anchor) {
    const bounds = getNodeBounds(anchor.id)
    if (!bounds.visible) return undefined

    const screen = getScreenGeometry(anchor.id)
    const fallback = measuredSize ?? getWidgetSizeHint(popupNodeId)
    const popupWidth = props.width ?? fallback.width
    const popupHeight = props.height ?? fallback.height
    const placement = props.placement ?? "bottom"

    type Placement = typeof placement
    const flipMap: Record<Placement, Placement> = {
      bottom: "top", top: "bottom", right: "left", left: "right",
    }

    const tryPlacement = (p: Placement): { x: number; y: number } => {
      switch (p) {
        case "top":
          return { x: bounds.screenX, y: bounds.screenY - popupHeight }
        case "bottom":
          return { x: bounds.screenX, y: bounds.screenY + bounds.height }
        case "right":
          return { x: bounds.screenX + bounds.width, y: bounds.screenY }
        case "left":
          return { x: bounds.screenX - popupWidth, y: bounds.screenY }
      }
    }

    let pos = tryPlacement(placement)

    if (screen.height > 0 && screen.width > 0) {
      const screenRight = screen.x + screen.width
      const screenBottom = screen.y + screen.height
      const flipped = flipMap[placement]

      const overflows = (p: { x: number; y: number }) =>
        p.x < screen.x || p.x + popupWidth > screenRight ||
        p.y < screen.y || p.y + popupHeight > screenBottom

      if (overflows(pos)) {
        const alt = tryPlacement(flipped)
        if (!overflows(alt)) {
          pos = alt
        }
      }

      // Clamp to screen bounds
      pos.x = Math.max(screen.x, Math.min(pos.x, screenRight - popupWidth))
      pos.y = Math.max(screen.y, Math.min(pos.y, screenBottom - popupHeight))
    }

    return pos
  }

  if (props.screenX != null && props.screenY != null) {
    return { x: props.screenX, y: props.screenY }
  }

  return undefined
}

export type PopupComposable = (children: Accessor<JSX.Element>) => JSX.Element

export function usePopup(source: PopupSource): PopupComposable {
  const read = toAccessor(source)

  return (children) => {
    const root = nativeRoot()

    const parentPopup = useContext(PopupOwnerContext)

    const node = createQtElement("window")
    const popupNode = node as unknown as QtNode

    // Force hidden before insertion — native window default is visible
    setQtProp(node, "visible", false, undefined)

    // Cascade dismiss: this popup's dismiss also dismisses parent chain unless stopped
    const dismissAll = () => {
      let propagationStopped = false
      const event: PopupDismissEvent = {
        stopPropagation() { propagationStopped = true },
      }
      read().onDismiss?.(event)
      if (!propagationStopped) {
        parentPopup?.dismiss()
      }
    }

    // Window props — popup effect owns visible separately, so exclude it from spread
    const baseWindowProps = windowPropsFrom(read as unknown as Accessor<WindowProps>)
    const { visible: _v, ...windowPropsNoVisible } = Object.getOwnPropertyDescriptors(baseWindowProps)
    spreadQtProps(
      node,
      extendProps(
        Object.defineProperties({}, windowPropsNoVisible),
        {
          windowKind: getter(() => 1),
          transparentBackground: getter(() => true),
          onCloseRequested: getter(() => dismissAll),
        },
      ),
    )

    // Canvas fragment binding — popup window needs its own canvas to host fragment children
    const fragmentBinding = createCanvasFragmentBinding(popupNode.id)
    registerCanvasBinding(popupNode.id, fragmentBinding.root)

    onCleanup(() => {
      unregisterCanvasBinding(popupNode.id)
      destroyCanvasFragmentBinding(popupNode.id)
    })

    // Wrap children with PopupOwnerContext so nested popups can find their parent
    const popupOwnerState: PopupOwnerState = {
      get id() { return popupNode.id },
      dismiss: dismissAll,
    }

    createComponent(CanvasScopeContext.Provider, {
      value: { canvasNodeId: popupNode.id, root: fragmentBinding.root },
      get children() {
        const resolved = resolveChildren(children)
        insertInto(fragmentBinding.root, () =>
          createComponent(PopupOwnerContext.Provider, {
            value: popupOwnerState,
            get children() { return resolved() },
          }),
        )
        return undefined
      },
    })

    // Portal: insert popup window into root, not into the JSX parent
    insertQtNode(root, node)

    // Single effect owns position + transient owner + visibility
    let previousVisible = false
    let positionRetryTimer: ReturnType<typeof setTimeout> | undefined

    const applyPosition = (measured?: { width: number; height: number }) => {
      const props = read()
      const pos = computePopupPosition(props, popupNode.id, measured)
      if (pos) {
        setQtProp(node, "screenX", pos.x, undefined)
        setQtProp(node, "screenY", pos.y, undefined)
      }
    }

    createEffect(() => {
      const props = read()
      const nextVisible = props.visible ?? false

      if (positionRetryTimer != null) {
        clearTimeout(positionRetryTimer)
        positionRetryTimer = undefined
      }

      if (nextVisible) {
        // Hug content: run intrinsic layout pass and size window to content
        const contentSize = canvasComputeIntrinsicSize(popupNode.id)
        const measured = contentSize
          ? {
              width: Math.ceil(props.width ?? contentSize.width),
              height: Math.ceil(props.height ?? contentSize.height),
            }
          : undefined

        if (measured) {
          setQtProp(node, "width", measured.width, undefined)
          setQtProp(node, "height", measured.height, undefined)
        }

        applyPosition(measured)
        positionRetryTimer = setTimeout(() => applyPosition(measured), 0)

        // Transient owner: prefer parent popup, then anchor's window
        const ownerId = parentPopup?.id ?? props.anchor?.id
        if (ownerId != null) {
          setWindowTransientOwner(popupNode.id, ownerId)
        }
      }

      setQtProp(node, "visible", nextVisible, previousVisible)

      if (!nextVisible && previousVisible && props.anchor) {
        focusWidget(props.anchor.id)
      }

      previousVisible = nextVisible
    })

    onCleanup(() => {
      if (positionRetryTimer != null) {
        clearTimeout(positionRetryTimer)
      }
      root.removeChild(node)
      node.destroy()
    })

    return null!
  }
}

export function createPopup(
  source: PopupSource,
  body: () => JSX.Element,
): { render: () => JSX.Element; dispose: () => void; open: () => void } {
  const read = toAccessor(source)
  const [disposed, setDisposed] = createSignal(true)

  const dispose = () => {
    setDisposed(true)
  }

  const open = () => {
    setDisposed(false)
  }

  const PopupMount: Component = () => {
    const effectiveProps = (): PopupProps => {
      const base = read()
      const controlled = base.visible != null
      return {
        ...base,
        visible: base.visible ?? !disposed(),
        onDismiss(event) {
          if (!controlled) {
            setDisposed(true)
          }
          base.onDismiss?.(event)
        },
      }
    }

    return usePopup(effectiveProps)(body)
  }

  const render = () => {
    return createComponent(PopupMount, {})
  }

  return { render, dispose, open }
}
