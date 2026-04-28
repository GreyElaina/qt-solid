import {
  createComponent,
  onCleanup,
  type Component,
  type JSX,
} from "solid-js"
import type { QtNode } from "@qt-solid/core/native"

import {
  createElement as createQtElement,
  insert as insertInto,
  spread as spreadQtProps,
  createCanvasFragmentBinding,
  destroyCanvasFragmentBinding,
  registerCanvasBinding,
  unregisterCanvasBinding,
  CanvasScopeContext,
} from "../runtime/renderer.ts"
import { getter } from "./props.ts"
import type { WidgetProps } from "./types.ts"

export interface CanvasProps extends WidgetProps {
  children?: JSX.Element
}

export const Canvas: Component<CanvasProps> = (props) => {
  const node = createQtElement("canvas")
  const canvasNode = node as unknown as QtNode

  spreadQtProps(
    node,
    Object.defineProperties({}, {
      width: getter(() => props.width),
      height: getter(() => props.height),
      minWidth: getter(() => props.minWidth),
      minHeight: getter(() => props.minHeight),
      grow: getter(() => props.flexGrow),
      shrink: getter(() => props.flexShrink),
      enabled: getter(() => props.enabled),
      hidden: getter(() => (props as any).hidden),
    }),
  )

  const fragmentBinding = createCanvasFragmentBinding(canvasNode.id)
  registerCanvasBinding(canvasNode.id, fragmentBinding.root)

  onCleanup(() => {
    unregisterCanvasBinding(canvasNode.id)
    destroyCanvasFragmentBinding(canvasNode.id)
  })

  createComponent(CanvasScopeContext.Provider, {
    value: { canvasNodeId: canvasNode.id, root: fragmentBinding.root },
    get children() {
      insertInto(fragmentBinding.root, () => props.children)
      return undefined
    },
  })

  return node
}
