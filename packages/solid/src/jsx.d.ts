import type { JSX as SolidJsx } from "solid-js"

import type { QtIntrinsicElements } from "@qt-solid/core-widgets/qt-intrinsics"

declare global {
  namespace JSX {
    type Element = SolidJsx.Element

    interface ElementChildrenAttribute {
      children: {}
    }

    interface IntrinsicElements extends QtIntrinsicElements {}
  }
}

export {}
