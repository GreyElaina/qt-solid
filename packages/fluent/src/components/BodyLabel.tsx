import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface BodyLabelProps {
  text?: string
  color?: string
}

export const BodyLabel: Component<BodyLabelProps> = (props) => {
  const theme = useTheme()

  return (
    <text
      text={props.text ?? ""}
      fontSize={theme().fontSizeBody}
      color={props.color ?? theme().foregroundPrimary}
    />
  )
}
