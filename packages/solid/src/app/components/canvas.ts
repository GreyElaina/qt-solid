import {
  createComponent,
  onCleanup,
  useContext,
  type Component,
  type JSX,
} from "solid-js"

import {
  insert as insertInto,
  createNativeWidget,
  spreadWidgetProps,
  createCanvasFragmentBinding,
  destroyCanvasFragmentBinding,
  registerCanvasBinding,
  unregisterCanvasBinding,
  CanvasScopeContext,
} from "../../runtime/renderer.ts"
import { getter } from "../props.ts"
import type { WidgetProps } from "../types.ts"

export interface CanvasProps extends WidgetProps {
  children?: JSX.Element
}

export const Canvas: Component<CanvasProps> = (props) => {
  const widgetNode = createNativeWidget()

  spreadWidgetProps(
    widgetNode,
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

  // Insert canvas widget into parent window widget via outer CanvasScope
  const outerScope = useContext(CanvasScopeContext)
  const parentQtNode = outerScope?.hostNode ?? null
  if (parentQtNode) {
    parentQtNode.insertChild(widgetNode.qtNode, null)
  }

  const fragmentBinding = createCanvasFragmentBinding(widgetNode.qtNode)
  registerCanvasBinding(widgetNode.id, fragmentBinding.root)

  onCleanup(() => {
    unregisterCanvasBinding(widgetNode.id)
    destroyCanvasFragmentBinding(widgetNode.id)
    if (parentQtNode) {
      parentQtNode.removeChild(widgetNode.qtNode)
    }
    widgetNode.destroy()
  })

  createComponent(CanvasScopeContext.Provider, {
    value: { hostNode: widgetNode.qtNode, root: fragmentBinding.root },
    get children() {
      insertInto(fragmentBinding.root, () => props.children)
      return undefined
    },
  })

  return undefined as unknown as JSX.Element
}
