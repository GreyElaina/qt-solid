import type { Component, JSX } from "solid-js"
import { createPopup } from "@qt-solid/solid"
import type { PopupDismissEvent } from "@qt-solid/solid"
import type { QtNode } from "@qt-solid/core/native"

import { useTheme } from "../theme.ts"

export interface FlyoutProps {
  anchor?: QtNode
  visible?: boolean
  placement?: "bottom" | "top" | "right" | "left"
  onDismiss?: () => void
  children?: JSX.Element
  width?: number
  height?: number
}

export const Flyout: Component<FlyoutProps> = (props) => {
  const theme = useTheme()

  const popup = createPopup(
    () => ({
      visible: props.visible ?? false,
      anchor: props.anchor,
      placement: props.placement ?? "bottom",
      width: props.width,
      height: props.height,
      onDismiss: (_e: PopupDismissEvent) => props.onDismiss?.(),
    }),
    () => (
      <rect
        fill={theme().backgroundDefault}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        cornerRadius={theme().radiusLg}
        w={props.width}
        h={props.height}
        padding={theme().spacingXl}
      >
        {props.children}
      </rect>
    ),
  )

  return popup.render()
}
