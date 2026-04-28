import type { Component } from "solid-js"

import { useTheme } from "../theme.ts"

export interface CaptionLabelProps {
  text?: string
  color?: string
}

export const CaptionLabel: Component<CaptionLabelProps> = (props) => {
  const theme = useTheme()

  return (
    <text
      text={props.text ?? ""}
      fontSize={theme().fontSizeCaption}
      color={props.color ?? theme().foregroundSecondary}
    />
  )
}
