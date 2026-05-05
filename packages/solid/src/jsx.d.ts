import type { FragmentRendererNode } from "./runtime/fragment.ts"
import type { NativeWidgetNode } from "./runtime/renderer.ts"
import type { QtIntrinsicElements } from "./qt-intrinsics.ts"

declare global {
  namespace JSX {
    type Element = FragmentRendererNode | NativeWidgetNode | undefined

    interface ElementChildrenAttribute {
      children: {}
    }

    interface IntrinsicElements extends QtIntrinsicElements {}
  }
}

export {}
