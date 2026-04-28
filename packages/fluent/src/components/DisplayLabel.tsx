import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface DisplayLabelProps {
  text?: string
  color?: string
}

export const DisplayLabel: Component<DisplayLabelProps> = (props) => {
  const theme = useTheme()

  return (
    <text
      text={props.text ?? ""}
      fontSize={theme().fontSizeDisplay}
      color={props.color ?? theme().foregroundPrimary}
    />
  )
}
