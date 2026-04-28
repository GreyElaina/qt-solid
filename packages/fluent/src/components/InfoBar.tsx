import { createSignal, type Component } from "solid-js"

import { useTheme } from "../theme.ts"

export type InfoBarSeverity = "info" | "success" | "warning" | "error"

export interface InfoBarProps {
  severity?: InfoBarSeverity
  title?: string
  message?: string
  closable?: boolean
  width?: number
  onClose?: () => void
}

const SEVERITY_COLORS: Record<InfoBarSeverity, string> = {
  info: "", // filled at runtime from theme
  success: "#0F7B0F",
  warning: "#FCE100",
  error: "#FF99A4",
}

const SEVERITY_ICONS: Record<InfoBarSeverity, string> = {
  info: "i",
  success: "\u2713",
  warning: "!",
  error: "\u00D7",
}

const ICON_SIZE = 20
const CLOSE_SIZE = 24

export const InfoBar: Component<InfoBarProps> = (props) => {
  const theme = useTheme()
  const [closeHovered, setCloseHovered] = createSignal(false)
  const [closePressed, setClosePressed] = createSignal(false)

  const severity = () => props.severity ?? "info"

  const severityColor = () => {
    if (severity() === "info") return theme().accentDefault
    return SEVERITY_COLORS[severity()]
  }

  const iconFg = () => {
    const s = severity()
    if (s === "warning") return "#000000"
    return "#FFFFFF"
  }

  const closeBg = () => {
    if (closePressed()) return theme().controlPressed
    if (closeHovered()) return theme().controlHover
    return "transparent"
  }

  return (
    <rect
      flexDirection="row"
      width={props.width ?? 360}
      fill={theme().backgroundSecondary}
      stroke={theme().strokeDefault}
      strokeWidth={1}
      cornerRadius={theme().radiusMd}
    >
      {/* Left accent stripe */}
      <rect
        width={4}
        fill={severityColor()}
        cornerRadius={{ topLeft: theme().radiusMd, topRight: 0, bottomRight: 0, bottomLeft: theme().radiusMd }}
      />

      {/* Content area with padding */}
      <group
        flexDirection="row"
        alignItems="center"
        gap={theme().spacingMd}
        padding={theme().spacingLg}
        flexGrow={1}
      >
        {/* Severity icon */}
        <group width={ICON_SIZE} height={ICON_SIZE} alignItems="center" justifyContent="center">
          <circle
            position="absolute"
            r={ICON_SIZE / 2}
            fill={severityColor()}
          />
          <text
            text={SEVERITY_ICONS[severity()]}
            fontSize={theme().fontSizeCaption}
            color={iconFg()}
          />
        </group>

        {/* Title + message */}
        <group flexDirection="column" gap={theme().spacingXs} flexGrow={1} flexShrink={1}>
          {props.title && (
            <text
              text={props.title}
              fontSize={theme().fontSizeBody}
              color={theme().foregroundPrimary}
              fontWeight={700}
            />
          )}
          {props.message && (
            <text
              text={props.message}
              fontSize={theme().fontSizeBody}
              color={theme().foregroundSecondary}
            />
          )}
        </group>

        {/* Close button */}
        {props.closable && (
          <rect
            width={CLOSE_SIZE}
            height={CLOSE_SIZE}
            alignItems="center"
            justifyContent="center"
            fill={closeBg()}
            cornerRadius={theme().radiusSm}
            focusable
            onPointerEnter={() => setCloseHovered(true)}
            onPointerLeave={() => { setCloseHovered(false); setClosePressed(false) }}
            onPointerDown={() => setClosePressed(true)}
            onPointerUp={() => {
              if (closePressed()) props.onClose?.()
              setClosePressed(false)
            }}
            onClick={() => {}}
          >
            <text
              text={"\u00D7"}
              fontSize={theme().fontSizeBody}
              color={theme().foregroundSecondary}
            />
          </rect>
        )}
      </group>
    </rect>
  )
}
