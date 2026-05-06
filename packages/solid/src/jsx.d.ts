import type { FragmentRendererNode } from "./runtime/fragment.ts"
import type { NativeWidgetNode } from "./runtime/renderer.ts"
import type { MotionProps } from "./app/motion/types.ts"
import type {
  WindowIntrinsicProps,
  GroupProps,
  RectProps,
  CircleProps,
  TextProps,
  TextInputProps,
  PathProps,
  ImageProps,
  SpanProps,
  GridProps,
} from "./intrinsics.ts"

declare global {
  namespace JSX {
    type Element = FragmentRendererNode | NativeWidgetNode | undefined

    interface ElementChildrenAttribute {
      children: {}
    }

    interface IntrinsicAttributes extends MotionProps {}

    interface IntrinsicElements {
      window: WindowIntrinsicProps
      group: GroupProps
      rect: RectProps
      circle: CircleProps
      text: TextProps
      textinput: TextInputProps
      path: PathProps
      image: ImageProps
      span: SpanProps
      grid: GridProps
    }
  }
}

export {}
