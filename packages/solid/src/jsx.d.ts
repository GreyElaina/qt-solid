import type { FragmentRendererNode } from "./runtime/fragment.ts"
import type { NativeWidgetNode } from "./runtime/renderer.ts"
import type { QtIntrinsicElements } from "./qt-intrinsics.ts"
import type { MotionProps } from "./app/motion/types.ts"

declare global {
  namespace JSX {
    type Element = FragmentRendererNode | NativeWidgetNode | undefined

    interface ElementChildrenAttribute {
      children: {}
    }

    interface IntrinsicAttributes extends MotionProps {}

    interface IntrinsicElements extends QtIntrinsicElements {}
  }
}

export {}
