// ---------------------------------------------------------------------------
// PreviewShell — wraps a previewed component with background, centering,
// title bar, and size indicator. Built with qt-solid primitives.
// ---------------------------------------------------------------------------

import { createSignal, type JSX } from "solid-js"

export interface PreviewShellProps {
  componentName: string
  children: JSX.Element
}

// Catppuccin Mocha palette
const SHELL_BG = "#1e1e2e"
const CANVAS_BG = "#181825"
const TITLE_BG = "#313244"
const TITLE_FG = "#cdd6f4"
const BORDER = "#45475a"
const DIM_FG = "#585b70"
const ACCENT = "#89b4fa"

export function PreviewShell(props: PreviewShellProps): JSX.Element {
  const [width, setWidth] = createSignal(0)
  const [height, setHeight] = createSignal(0)

  const sizeLabel = () => {
    const w = width()
    const h = height()
    if (w === 0 && h === 0) return ""
    return `${Math.round(w)} × ${Math.round(h)}`
  }

  return (
    <rect fill={SHELL_BG} flexGrow={1} flexDirection="column">
      {/* Title bar */}
      <rect
        fill={TITLE_BG}
        height={36}
        flexDirection="row"
        alignItems="center"
        paddingLeft={12}
        paddingRight={12}
        gap={8}
      >
        <rect fill={ACCENT} width={8} height={8} cornerRadius={4} />
        <text
          text={props.componentName}
          fontSize={12}
          fontWeight={500}
          color={TITLE_FG}
          flexGrow={1}
        />
        <text text={sizeLabel()} fontSize={10} color={DIM_FG} />
        <text text="Preview" fontSize={10} color={DIM_FG} />
      </rect>

      {/* Separator */}
      <rect fill={BORDER} height={1} />

      {/* Canvas area */}
      <rect
        fill={CANVAS_BG}
        flexGrow={1}
        alignItems="center"
        justifyContent="center"
        padding={32}
      >
        <rect
          flexDirection="column"
          alignItems="center"
          gap={12}
        >
          {/* Component stage */}
          <rect
            flexDirection="column"
            cornerRadius={6}
            stroke={BORDER}
            strokeWidth={1}
            padding={16}
            fill={SHELL_BG}
            onLayout={(e: { x: number; y: number; width: number; height: number }) => {
              setWidth(e.width)
              setHeight(e.height)
            }}
          >
            {props.children}
          </rect>

          {/* Component name label */}
          <text
            text={`‹${props.componentName}›`}
            fontSize={10}
            color={DIM_FG}
          />
        </rect>
      </rect>
    </rect>
  ) as JSX.Element
}
