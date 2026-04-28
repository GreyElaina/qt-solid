import type { QtRendererNode } from "./runtime/renderer.ts"
import type { QtIntrinsicElements } from "./qt-intrinsics.ts"

declare global {
  namespace JSX {
    type Element = QtRendererNode

    interface ElementChildrenAttribute {
      children: {}
    }

    interface IntrinsicElements extends QtIntrinsicElements {}
  }
}

export {}
