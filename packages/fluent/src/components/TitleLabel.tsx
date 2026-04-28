import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface TitleLabelProps {
  text?: string
  color?: string
}

export const TitleLabel: Component<TitleLabelProps> = (props) => {
  const theme = useTheme()

  return (
    <text
      text={props.text ?? ""}
      fontSize={theme().fontSizeTitle}
      color={props.color ?? theme().foregroundPrimary}
    />
  )
}
