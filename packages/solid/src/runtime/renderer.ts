import { createRenderer } from "solid-js/universal"

import type { QtSolidOwnerMetadata } from "../devtools/owner-metadata.ts"

export type QtFlexDirection = "column" | "row"
export type QtAlignItems = "flex-start" | "center" | "flex-end" | "stretch"
export type QtJustifyContent = "flex-start" | "center" | "flex-end"

export interface QtRendererNode {
  readonly id: number
  readonly parent: QtRendererNode | null
  readonly firstChild: QtRendererNode | null
  readonly nextSibling: QtRendererNode | null
  isTextNode(): boolean
  insertChild(child: QtRendererNode, anchor?: QtRendererNode | null): void
  removeChild(child: QtRendererNode): void
  destroy(): void
}

export interface QtRendererDebugMetadata {
  owner?: QtSolidOwnerMetadata | null
}

export interface QtRendererBinding<Node extends QtRendererNode = QtRendererNode> {
  readonly root: Node
  createElement(type: string): Node
  createTextNode(value: string): Node
  replaceText(node: Node, value: string): void
  insertChild(parent: Node, child: Node, anchor?: Node): void
  removeChild(parent: Node, child: Node): void
  getParent(node: Node): Node | undefined
  getFirstChild(node: Node): Node | undefined
  getNextSibling(node: Node): Node | undefined
  isTextNode(node: Node): boolean
  patchProp(node: Node, key: string, prev: unknown, next: unknown): void
  attachDebugMetadata?(node: Node, metadata: QtRendererDebugMetadata): void
}

export function createQtRenderer<Node extends QtRendererNode>(binding: QtRendererBinding<Node>) {
  return createRenderer<Node>({
    createElement(type) {
      return binding.createElement(type)
    },
    createTextNode(value) {
      return binding.createTextNode(value)
    },
    replaceText(node, value) {
      binding.replaceText(node, value)
    },
    setProperty(node, name, value, prev) {
      binding.patchProp(node, name, prev, value)
    },
    insertNode(parent, node, anchor) {
      binding.insertChild(parent, node, anchor ?? undefined)
    },
    removeNode(parent, node) {
      binding.removeChild(parent, node)
    },
    getParentNode(node) {
      return binding.getParent(node)
    },
    getFirstChild(node) {
      return binding.getFirstChild(node)
    },
    getNextSibling(node) {
      return binding.getNextSibling(node)
    },
    isTextNode(node) {
      return binding.isTextNode(node)
    },
  })
}
