import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"
import { describeArc } from "../primitives/arc.ts"

export interface ProgressRingProps {
  value?: number
  size?: number
  strokeWidth?: number
  showText?: boolean
  paused?: boolean
  error?: boolean
}

export const ProgressRing: Component<ProgressRingProps> = (props) => {
  const theme = useTheme()
  const sz = () => props.size ?? 80
  const sw = () => props.strokeWidth ?? 6
  const v = () => Math.max(0, Math.min(100, props.value ?? 0))
  const half = () => sz() / 2
  const r = () => (sz() - sw()) / 2

  const barColor = () => {
    if (props.error) return "#FF99A4"
    if (props.paused) return "#FCE100"
    return theme().accentDefault
  }

  const bgArc = () => describeArc(half(), half(), r(), 0, 359.99)
  const fgArc = () => {
    const deg = (v() / 100) * 360
    return deg > 0.5 ? describeArc(half(), half(), r(), 0, deg) : ""
  }

  return (
    <group width={sz()} height={sz()}>
      <path d={bgArc()} stroke={theme().strokeDefault} strokeWidth={sw()} />
      {v() > 0 && <path d={fgArc()} stroke={barColor()} strokeWidth={sw()} />}
      {props.showText && (
        <text
          x={half()}
          y={half()}
          text={`${Math.round(v())}%`}
          fontSize={theme().fontSizeBody}
          color={theme().foregroundPrimary}
        />
      )}
    </group>
  )
}
