import type { Component, JSX } from "solid-js"

import { createRuntimeElement } from "./props.ts"
import type {
  CanvasGroupProps,
  CanvasRectProps,
  CanvasCircleProps,
  CanvasTextProps,
  CanvasTextInputProps,
  CanvasPathProps,
  CanvasImageProps,
  CanvasSpanProps,
} from "../qt-intrinsics.ts"

export function defineIntrinsicComponent<Props extends object>(
  type: string,
): Component<Props> {
  return (props) => createRuntimeElement(type, props as Record<string, unknown>)
}

// Pre-defined intrinsic components — use these instead of defineIntrinsicComponent()
export const Group = defineIntrinsicComponent<CanvasGroupProps>("group")
export const Rect = defineIntrinsicComponent<CanvasRectProps>("rect")
export const Circle = defineIntrinsicComponent<CanvasCircleProps>("circle")
export const Text = defineIntrinsicComponent<CanvasTextProps>("text")
export const TextInput = defineIntrinsicComponent<CanvasTextInputProps>("textinput")
export const Path = defineIntrinsicComponent<CanvasPathProps>("path")
export const Img = defineIntrinsicComponent<CanvasImageProps>("image")
export const Span = defineIntrinsicComponent<CanvasSpanProps>("span")
