import { Show, type Component } from "solid-js"
import { createPopup } from "@qt-solid/solid"
import type { PopupDismissEvent } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"
import { Button } from "./Button.tsx"
import { TransparentButton } from "./TransparentButton.tsx"

export interface DialogProps {
  visible?: boolean
  title?: string
  content?: string
  primaryText?: string
  secondaryText?: string
  onPrimary?: () => void
  onSecondary?: () => void
  onDismiss?: () => void
  screenX?: number
  screenY?: number
  width?: number
}

const DIALOG_WIDTH = 400

export const Dialog: Component<DialogProps> = (props) => {
  const theme = useTheme()
  const w = () => props.width ?? DIALOG_WIDTH

  const popup = createPopup(
    () => ({
      visible: props.visible ?? false,
      screenX: props.screenX,
      screenY: props.screenY,
      width: w(),
      onDismiss: (_e: PopupDismissEvent) => props.onDismiss?.(),
    }),
    () => (
      <rect
        fill={theme().backgroundDefault}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        cornerRadius={theme().radiusLg}
        width={w()}
        flexDirection="column"
        padding={theme().spacingXl}
        gap={theme().spacingLg}
      >
        <Show when={props.title}>
          <text
            text={props.title!}
            fontSize={theme().fontSizeSubtitle}
            color={theme().foregroundPrimary}
          />
        </Show>
        <Show when={props.content}>
          <text
            text={props.content!}
            fontSize={theme().fontSizeBody}
            color={theme().foregroundSecondary}
          />
        </Show>
        <group
          flexDirection="row"
          justifyContent="flex-end"
          gap={theme().spacingMd}
        >
          <Show when={props.secondaryText}>
            <TransparentButton
              onClick={() => {
                props.onSecondary?.()
                props.onDismiss?.()
              }}
            >
              {props.secondaryText!}
            </TransparentButton>
          </Show>
          <Show when={props.primaryText}>
            <Button
              accent
              onClick={() => {
                props.onPrimary?.()
                props.onDismiss?.()
              }}
            >
              {props.primaryText!}
            </Button>
          </Show>
        </group>
      </rect>
    ),
  )

  return popup.render()
}
