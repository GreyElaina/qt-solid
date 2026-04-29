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
} from "@qt-solid/core/native"

import {
  createElement as createQtElement,
  insertNode as insertQtNode,
  setProp as setQtProp,
  spread as spreadQtProps,
  nativeRoot,
} from "../../runtime/renderer.ts"

import { extendProps, getter, toAccessor, viewPropsFrom } from "../props.ts"
import type { PopupDismissEvent, PopupProps, PopupSource } from "./types.ts"

interface PopupOwnerState {
  readonly id: number
  dismiss(): void
}

const PopupOwnerContext = createContext<PopupOwnerState | null>(null)

function computePopupPosition(
  props: PopupProps,
  popupNodeId: number,
): { x: number; y: number } | undefined {
  const anchor = props.anchor
  if (anchor) {
    const bounds = getNodeBounds(anchor.id)
    if (!bounds.visible) return undefined

    const screen = getScreenGeometry(anchor.id)
    const sizeHint = getWidgetSizeHint(popupNodeId)
    const popupWidth = props.width ?? sizeHint.width
    const popupHeight = props.height ?? sizeHint.height
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

    // Wrap children with PopupOwnerContext so nested popups can find their parent
    const popupOwnerState: PopupOwnerState = {
      get id() { return popupNode.id },
      dismiss: dismissAll,
    }
    const resolved = resolveChildren(children)
    const wrappedChildren = () => createComponent(PopupOwnerContext.Provider, {
      value: popupOwnerState,
      get children() { return resolved() },
    })

    // Build popup props lazily — never spread `read()` eagerly to avoid
    // reading `children` inside non-children effects (which would re-mount
    // nested Popup components on every prop change).
    spreadQtProps(
      node,
      extendProps(viewPropsFrom(read, wrappedChildren), {
        windowKind: getter(() => 1),
        transparentBackground: getter(() => true),
        onCloseRequested: getter(() => dismissAll),
      }),
    )

    // Portal: insert popup window into root, not into the JSX parent
    insertQtNode(root, node)

    // Single effect owns position + transient owner + visibility
    let previousVisible = false
    let positionRetryTimer: ReturnType<typeof setTimeout> | undefined

    const applyPosition = () => {
      const props = read()
      const pos = computePopupPosition(props, popupNode.id)
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
        applyPosition()
        positionRetryTimer = setTimeout(applyPosition, 0)

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
      const visible = base.visible ?? !disposed()
      return { ...base, visible }
    }

    return usePopup(effectiveProps)(body)
  }

  const render = () => {
    return createComponent(PopupMount, {})
  }

  return { render, dispose, open }
}
