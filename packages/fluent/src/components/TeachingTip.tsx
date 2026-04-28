import { Show, type Component } from "solid-js"
import { createPopup } from "@qt-solid/solid"
import type { PopupDismissEvent } from "@qt-solid/solid"

import { useTheme } from "../theme.ts"
import { Button } from "./Button.tsx"
import { TransparentButton } from "./TransparentButton.tsx"

export interface TeachingTipProps {
  anchor?: { readonly id: number }
  visible?: boolean
  placement?: "bottom" | "top" | "right" | "left"
  title?: string
  subtitle?: string
  actionText?: string
  closeText?: string
  onAction?: () => void
  onClose?: () => void
}

const TIP_WIDTH = 320

export const TeachingTip: Component<TeachingTipProps> = (props) => {
  const theme = useTheme()

  const popup = createPopup(
    () => ({
      visible: props.visible ?? false,
      anchor: props.anchor,
      placement: props.placement ?? "bottom",
      width: TIP_WIDTH,
      onDismiss: (_e: PopupDismissEvent) => props.onClose?.(),
    }),
    () => (
      <rect
        fill={theme().backgroundDefault}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        cornerRadius={theme().radiusLg}
        width={TIP_WIDTH}
        flexDirection="column"
        padding={theme().spacingXl}
        gap={theme().spacingMd}
      >
        <Show when={props.title}>
          <text
            text={props.title!}
            fontSize={theme().fontSizeSubtitle}
            color={theme().foregroundPrimary}
          />
        </Show>
        <Show when={props.subtitle}>
          <text
            text={props.subtitle!}
            fontSize={theme().fontSizeBody}
            color={theme().foregroundSecondary}
          />
        </Show>
        <group
          flexDirection="row"
          justifyContent="flex-end"
          gap={theme().spacingMd}
        >
          <Show when={props.actionText}>
            <Button accent onClick={props.onAction}>
              {props.actionText!}
            </Button>
          </Show>
          <TransparentButton onClick={props.onClose}>
            {props.closeText ?? "Got it"}
          </TransparentButton>
        </group>
      </rect>
    ),
  )

  return popup.render()
}
