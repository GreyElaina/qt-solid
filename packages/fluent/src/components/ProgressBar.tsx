import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface ProgressBarProps {
  value?: number
  width?: number
  height?: number
  paused?: boolean
  error?: boolean
}

export const ProgressBar: Component<ProgressBarProps> = (props) => {
  const theme = useTheme()

  const w = () => props.width ?? 200
  const h = () => props.height ?? 4
  const v = () => Math.max(0, Math.min(100, props.value ?? 0))
  const barWidth = () => (v() / 100) * w()
  const r = () => h() / 2

  const barColor = () => {
    if (props.error) return "#FF99A4"
    if (props.paused) return "#FCE100"
    return theme().accentDefault
  }

  return (
    <group width={w()} height={h()}>
      <rect
        position="absolute"
        x={0}
        y={0}
        width={w()}
        height={h()}
        fill={theme().strokeDefault}
        opacity={0.3}
        cornerRadius={r()}
      />
      <rect
        position="absolute"
        x={0}
        y={0}
        width={barWidth()}
        height={h()}
        fill={barColor()}
        cornerRadius={r()}
      />
    </group>
  )
}
