import { createContext, useContext } from "solid-js"
import { createRenderer } from "solid-js/universal"

import { currentQtSolidOwnerMetadata, withQtOwnerFrame } from "../devtools/owner-metadata.ts"
import type { QtRendererBinding, QtRendererNode } from "./renderer.ts"

type QtSolidBinding = QtRendererBinding<QtRendererNode>

export const QtRendererBindingContext = createContext<QtSolidBinding | null>(null)

const nodeBindings = new WeakMap<QtRendererNode, QtSolidBinding>()

function useCurrentBinding(): QtSolidBinding {
  const binding = useContext(QtRendererBindingContext)
  if (!binding) {
    throw new Error("Qt Solid runtime used outside renderQt")
  }
  return binding
}

function bindingFor(node: QtRendererNode): QtSolidBinding {
  const binding = nodeBindings.get(node)
  if (!binding) {
    throw new Error(`Qt Solid node ${node.id} is missing renderer binding`)
  }
  return binding
}

export function rememberQtRendererNode<Node extends QtRendererNode>(
  node: Node,
  binding: QtSolidBinding,
): Node {
  nodeBindings.set(node, binding)
  return node
}

function rememberMaybe<Node extends QtRendererNode>(
  node: Node | undefined,
  binding: QtSolidBinding,
): Node | undefined {
  return node ? rememberQtRendererNode(node, binding) : undefined
}

const renderer = createRenderer<QtRendererNode>({
  createElement(type) {
    const binding = useCurrentBinding()
    const node = rememberQtRendererNode(binding.createElement(type), binding)
    binding.attachDebugMetadata?.(node, {
      owner: currentQtSolidOwnerMetadata(),
    })
    return node
  },
  createTextNode(value) {
    const binding = useCurrentBinding()
    const node = rememberQtRendererNode(binding.createTextNode(String(value)), binding)
    binding.attachDebugMetadata?.(node, {
      owner: currentQtSolidOwnerMetadata(),
    })
    return node
  },
  replaceText(node, value) {
    bindingFor(node).replaceText(node, value)
  },
  setProperty(node, name, value, prev) {
    bindingFor(node).patchProp(node, name, prev, value)
  },
  insertNode(parent, node, anchor) {
    bindingFor(parent).insertChild(parent, node, anchor ?? undefined)
  },
  removeNode(parent, node) {
    bindingFor(parent).removeChild(parent, node)
  },
  getParentNode(node) {
    const binding = bindingFor(node)
    return rememberMaybe(binding.getParent(node), binding)
  },
  getFirstChild(node) {
    const binding = bindingFor(node)
    return rememberMaybe(binding.getFirstChild(node), binding)
  },
  getNextSibling(node) {
    const binding = bindingFor(node)
    return rememberMaybe(binding.getNextSibling(node), binding)
  },
  isTextNode(node) {
    return bindingFor(node).isTextNode(node)
  },
})

export const {
  render: _render,
  effect,
  memo,
  createElement,
  createTextNode,
  insertNode,
  insert,
  spread,
  setProp,
  mergeProps,
  use,
} = renderer

const createComponentBase = renderer.createComponent

export const createComponent = ((...args: Parameters<typeof createComponentBase>) => {
  const [component, props] = args
  return withQtOwnerFrame(component, props, () => createComponentBase(...args))
}) as typeof renderer.createComponent
