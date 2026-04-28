import type { Component, JSX } from "solid-js"

import { createRuntimeElement } from "./props.ts"

export function defineIntrinsicComponent<Props extends object>(
  type: string,
): Component<Props> {
  return (props) => createRuntimeElement(type, props as Record<string, unknown>)
}
