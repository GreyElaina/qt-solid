import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export type InfoLevel = "info" | "success" | "attention" | "warning" | "error"

export interface InfoBadgeProps {
  text?: string
  level?: InfoLevel
  width?: number
  height?: number
}

const LEVEL_COLORS: Record<InfoLevel, string> = {
  info: "",
  success: "#0F7B0F",
  attention: "#FCE100",
  warning: "#9D5D00",
  error: "#FF99A4",
}

const DARK_TEXT_LEVELS = new Set<InfoLevel>(["attention", "warning"])

export const InfoBadge: Component<InfoBadgeProps> = (props) => {
  const theme = useTheme()

  const level = () => props.level ?? "info"

  const bg = () => {
    if (level() === "info") return theme().accentDefault
    return LEVEL_COLORS[level()]
  }

  const fg = () => {
    if (DARK_TEXT_LEVELS.has(level())) return "#000000"
    return theme().foregroundOnAccent
  }

  const hasText = () => props.text != null && props.text.length > 0
  const h = () => hasText() ? (props.height ?? 16) : 8
  const w = () => {
    if (!hasText()) return 8
    if (props.width != null) return props.width
    return undefined
  }

  return (
    <rect
      width={w()}
      height={h()}
      alignItems="center"
      justifyContent="center"
      fill={bg()}
      cornerRadius={h() / 2}
      paddingLeft={hasText() ? 6 : 0}
      paddingRight={hasText() ? 6 : 0}
    >
      {hasText() && (
        <text
          text={props.text!}
          fontSize={theme().fontSizeCaption}
          color={fg()}
        />
      )}
    </rect>
  )
}
