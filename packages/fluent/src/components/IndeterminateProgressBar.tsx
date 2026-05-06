import type { Component } from "solid-js"

import { useMotionValue } from "@qt-solid/solid"
import { useTheme } from "../theme.ts"

export interface IndeterminateProgressBarProps {
  width?: number
  height?: number
  paused?: boolean
  error?: boolean
}

export const IndeterminateProgressBar: Component<IndeterminateProgressBarProps> = (props) => {
  const theme = useTheme()
  const w = () => props.width ?? 200
  const h = () => props.height ?? 4
  const rr = () => h() / 2

  const shortPos = useMotionValue({
    initial: -0.4,
    animate: 1.45,
    transition: { duration: 0.833, ease: "linear", repeat: Infinity, repeatType: "loop" },
  })

  const longPos = useMotionValue({
    initial: -0.6,
    animate: 1.75,
    transition: { duration: 1.167, ease: "ease-out", repeat: Infinity, repeatType: "loop", delay: 0.785 },
  })

  const barColor = () => {
    if (props.error) return "#FF99A4"
    if (props.paused) return "#FCE100"
    return theme().accentDefault
  }

  return (
    <rect w={w()} h={h()} clip={true} fill="#00000000">
      <rect absolute w={w()} h={h()} fill={theme().strokeDefault} opacity={0.3} cornerRadius={rr()} />
      <rect absolute transformX={shortPos() * w()} w={0.4 * w()} h={h()} fill={barColor()} cornerRadius={rr()} />
      <rect absolute transformX={longPos() * w()} w={0.6 * w()} h={h()} fill={barColor()} cornerRadius={rr()} />
    </rect>
  )
}
