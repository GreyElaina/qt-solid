import type { Component, JSX } from "solid-js"
import { useTooltip } from "@qt-solid/solid"
import type { CanvasNodeHandle } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"

export interface ToolTipProps {
  text: string
  placement?: "bottom" | "top" | "right" | "left"
  delay?: number
  children: JSX.Element
}

export const ToolTip: Component<ToolTipProps> = (props) => {
  const theme = useTheme()

  const tt = useTooltip({
    content: () => (
      <rect
        fill={theme().backgroundSecondary}
        cornerRadius={theme().radiusMd}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        padding={theme().spacingMd}
      >
        <text
          text={props.text}
          fontSize={theme().fontSizeCaption}
          color={theme().foregroundPrimary}
        />
      </rect>
    ),
    placement: props.placement ?? "bottom",
    hoverDelay: props.delay ?? 500,
  })

  return (
    <group
      ref={(node: CanvasNodeHandle) => tt.setAnchor({ id: node.canvasNodeId })}
      onPointerEnter={tt.onHoverEnter}
      onPointerLeave={tt.onHoverLeave}
    >
      {props.children}
      <tt.Portal />
    </group>
  )
}
