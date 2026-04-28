import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface SubtitleLabelProps {
  text?: string
  color?: string
}

export const SubtitleLabel: Component<SubtitleLabelProps> = (props) => {
  const theme = useTheme()

  return (
    <text
      text={props.text ?? ""}
      fontSize={theme().fontSizeSubtitle}
      color={props.color ?? theme().foregroundPrimary}
    />
  )
}
