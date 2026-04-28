import type { Component } from "solid-js"

import { useMotionValue } from "@qt-solid/solid"
import { useTheme } from "../theme.ts"
import { describeArc } from "../primitives/arc.ts"

export interface IndeterminateProgressRingProps {
  size?: number
  strokeWidth?: number
  paused?: boolean
  error?: boolean
}

export const IndeterminateProgressRing: Component<IndeterminateProgressRingProps> = (props) => {
  const theme = useTheme()
  const sz = () => props.size ?? 80
  const sw = () => props.strokeWidth ?? 6
  const half = () => sz() / 2
  const r = () => (sz() - sw()) / 2

  const startAngle = useMotionValue({
    animate: [0, 450, 1080],
    transition: { duration: 2, times: [0, 0.5, 1], ease: "linear", repeat: Infinity, repeatType: "loop" },
  })

  const spanAngle = useMotionValue({
    animate: [0, 180, 0],
    transition: { duration: 2, times: [0, 0.5, 1], ease: "linear", repeat: Infinity, repeatType: "loop" },
  })

  const barColor = () => {
    if (props.error) return "#FF99A4"
    if (props.paused) return "#FCE100"
    return theme().accentDefault
  }

  const arcPath = () => {
    const span = spanAngle()
    if (span < 1) return ""
    const sa = startAngle() % 360
    return describeArc(half(), half(), r(), sa, sa + span)
  }

  const bgArc = () => describeArc(half(), half(), r(), 0, 359.99)

  return (
    <group width={sz()} height={sz()}>
      <path d={bgArc()} stroke={theme().strokeDefault} strokeWidth={sw()} opacity={0.15} />
      {spanAngle() > 0 && <path d={arcPath()} stroke={barColor()} strokeWidth={sw()} />}
    </group>
  )
}
