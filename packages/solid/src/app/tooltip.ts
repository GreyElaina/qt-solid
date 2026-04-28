import {
  createComponent,
  createEffect,
  createSignal,
  onCleanup,
  type Accessor,
  type Component,
  type JSX,
} from "solid-js"
import type { QtNode } from "@qt-solid/core/native"
import {
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
} from "../runtime/renderer.ts"

import { tooltipPropsFrom } from "./props.ts"
import type { TooltipProps } from "./types.ts"

type Placement = "bottom" | "top" | "right" | "left"

function computeTooltipPosition(
  anchorId: number,
  tooltipNodeId: number,
  placement: Placement,
): { x: number; y: number } | undefined {
  const bounds = getNodeBounds(anchorId)
  if (!bounds.visible) return undefined

  const screen = getScreenGeometry(anchorId)
  const hint = getWidgetSizeHint(tooltipNodeId)
  const pw = hint.width
  const ph = hint.height

  const flipMap: Record<Placement, Placement> = {
    bottom: "top", top: "bottom", right: "left", left: "right",
  }

  const tryPlacement = (p: Placement): { x: number; y: number } => {
    switch (p) {
      case "top":    return { x: bounds.screenX, y: bounds.screenY - ph }
      case "bottom": return { x: bounds.screenX, y: bounds.screenY + bounds.height }
      case "right":  return { x: bounds.screenX + bounds.width, y: bounds.screenY }
      case "left":   return { x: bounds.screenX - pw, y: bounds.screenY }
    }
  }

  let pos = tryPlacement(placement)

  if (screen.height > 0 && screen.width > 0) {
    const sr = screen.x + screen.width
    const sb = screen.y + screen.height

    const overflows = (p: { x: number; y: number }) =>
      p.x < screen.x || p.x + pw > sr || p.y < screen.y || p.y + ph > sb

    if (overflows(pos)) {
      const alt = tryPlacement(flipMap[placement])
      if (!overflows(alt)) pos = alt
    }

    pos.x = Math.max(screen.x, Math.min(pos.x, sr - pw))
    pos.y = Math.max(screen.y, Math.min(pos.y, sb - ph))
  }

  return pos
}

export interface UseTooltipOptions {
  content: () => JSX.Element
  placement?: Placement
  hoverDelay?: number
  hideDelay?: number
}

export interface UseTooltipResult {
  onHoverEnter: () => void
  onHoverLeave: () => void
  setAnchor: (node: { readonly id: number }) => void
  Portal: Component
}

export function useTooltip(options: UseTooltipOptions): UseTooltipResult {
  const [hovered, setHovered] = createSignal(false)
  const [visible, setVisible] = createSignal(false)
  const [anchor, setAnchor] = createSignal<{ readonly id: number }>()

  const hoverDelay = () => options.hoverDelay ?? 500
  const hideDelay = () => options.hideDelay ?? 200

  let showTimer: ReturnType<typeof setTimeout> | undefined
  let hideTimer: ReturnType<typeof setTimeout> | undefined

  const clearTimers = () => {
    if (showTimer != null) { clearTimeout(showTimer); showTimer = undefined }
    if (hideTimer != null) { clearTimeout(hideTimer); hideTimer = undefined }
  }

  createEffect(() => {
    const isHovered = hovered()
    clearTimers()
    if (isHovered) {
      showTimer = setTimeout(() => setVisible(true), hoverDelay())
    } else {
      hideTimer = setTimeout(() => setVisible(false), hideDelay())
    }
  })

  onCleanup(clearTimers)

  const Portal: Component = () => {
    const root = nativeRoot()

    const node = createQtElement("window")
    const tooltipNode = node as unknown as QtNode

    setQtProp(node, "visible", false, undefined)
    spreadQtProps(node, tooltipPropsFrom(options.content))
    insertQtNode(root, node)

    let previousVisible = false
    createEffect(() => {
      const anchorNode = anchor()
      const nextVisible = visible() && anchorNode != null

      if (nextVisible && anchorNode) {
        const pos = computeTooltipPosition(
          anchorNode.id,
          tooltipNode.id,
          options.placement ?? "bottom",
        )
        if (pos) {
          setQtProp(node, "screenX", pos.x, undefined)
          setQtProp(node, "screenY", pos.y, undefined)
        }
        setWindowTransientOwner(tooltipNode.id, anchorNode.id)
      }

      setQtProp(node, "visible", nextVisible, previousVisible)
      previousVisible = nextVisible
    })

    onCleanup(() => {
      root.removeChild(node)
      node.destroy()
    })

    return null!
  }

  return {
    onHoverEnter: () => setHovered(true),
    onHoverLeave: () => setHovered(false),
    setAnchor,
    Portal,
  }
}
