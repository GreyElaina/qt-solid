import { createContext, useContext, type Accessor } from "solid-js"

// ---------------------------------------------------------------------------
// Design tokens — Fluent Design System (WinUI 3 reference palette)
// ---------------------------------------------------------------------------

export interface FluentTokens {
  // Accent
  readonly accentDefault: string
  readonly accentHover: string
  readonly accentPressed: string
  readonly accentDisabled: string

  // Foreground
  readonly foregroundPrimary: string
  readonly foregroundSecondary: string
  readonly foregroundDisabled: string
  readonly foregroundOnAccent: string

  // Background
  readonly backgroundDefault: string
  readonly backgroundSecondary: string
  readonly backgroundTertiary: string
  readonly backgroundDisabled: string

  // Stroke / border
  readonly strokeDefault: string
  readonly strokeDisabled: string
  readonly strokeFocus: string

  // Control
  readonly controlDefault: string
  readonly controlHover: string
  readonly controlPressed: string
  readonly controlDisabled: string

  // Spacing
  readonly spacingXs: number
  readonly spacingSm: number
  readonly spacingMd: number
  readonly spacingLg: number
  readonly spacingXl: number

  // Border radius
  readonly radiusSm: number
  readonly radiusMd: number
  readonly radiusLg: number
  readonly radiusXl: number
  readonly radiusCircular: number

  // Font sizes
  readonly fontSizeCaption: number
  readonly fontSizeBody: number
  readonly fontSizeSubtitle: number
  readonly fontSizeTitle: number
  readonly fontSizeDisplay: number

  // Focus ring
  readonly focusStrokeWidth: number
  readonly focusStrokeOuter: string
  readonly focusStrokeInner: string
}

// ---------------------------------------------------------------------------
// Dark theme tokens (WinUI 3 dark reference)
// ---------------------------------------------------------------------------

export const fluentDark: FluentTokens = {
  accentDefault: "#60CDFF",
  accentHover: "#78D8FF",
  accentPressed: "#4BB8E8",
  accentDisabled: "#FFFFFF28",

  foregroundPrimary: "#FFFFFF",
  foregroundSecondary: "#FFFFFFB3",
  foregroundDisabled: "#FFFFFF5C",
  foregroundOnAccent: "#000000",

  backgroundDefault: "#202020",
  backgroundSecondary: "#2D2D2D",
  backgroundTertiary: "#383838",
  backgroundDisabled: "#FFFFFF0F",

  strokeDefault: "#FFFFFF14",
  strokeDisabled: "#FFFFFF28",
  strokeFocus: "#FFFFFF",

  controlDefault: "#FFFFFF0F",
  controlHover: "#FFFFFF14",
  controlPressed: "#FFFFFF08",
  controlDisabled: "#FFFFFF0A",

  spacingXs: 2,
  spacingSm: 4,
  spacingMd: 8,
  spacingLg: 12,
  spacingXl: 16,

  radiusSm: 2,
  radiusMd: 4,
  radiusLg: 8,
  radiusXl: 12,
  radiusCircular: 9999,

  fontSizeCaption: 12,
  fontSizeBody: 14,
  fontSizeSubtitle: 20,
  fontSizeTitle: 28,
  fontSizeDisplay: 40,

  focusStrokeWidth: 2,
  focusStrokeOuter: "#FFFFFF",
  focusStrokeInner: "#000000",
}

// ---------------------------------------------------------------------------
// Light theme tokens
// ---------------------------------------------------------------------------

export const fluentLight: FluentTokens = {
  accentDefault: "#005FB8",
  accentHover: "#0067C0",
  accentPressed: "#003D7A",
  accentDisabled: "#00000037",

  foregroundPrimary: "#1A1A1A",
  foregroundSecondary: "#00000073",
  foregroundDisabled: "#0000005C",
  foregroundOnAccent: "#FFFFFF",

  backgroundDefault: "#F3F3F3",
  backgroundSecondary: "#EEEEEE",
  backgroundTertiary: "#F9F9F9",
  backgroundDisabled: "#0000000F",

  strokeDefault: "#0000002E",
  strokeDisabled: "#0000000F",
  strokeFocus: "#000000",

  controlDefault: "#FFFFFFB3",
  controlHover: "#F9F9F980",
  controlPressed: "#F9F9F94D",
  controlDisabled: "#F9F9F94D",

  spacingXs: 2,
  spacingSm: 4,
  spacingMd: 8,
  spacingLg: 12,
  spacingXl: 16,

  radiusSm: 2,
  radiusMd: 4,
  radiusLg: 8,
  radiusXl: 12,
  radiusCircular: 9999,

  fontSizeCaption: 12,
  fontSizeBody: 14,
  fontSizeSubtitle: 20,
  fontSizeTitle: 28,
  fontSizeDisplay: 40,

  focusStrokeWidth: 2,
  focusStrokeOuter: "#000000",
  focusStrokeInner: "#FFFFFF",
}

// ---------------------------------------------------------------------------
// Solid context
// ---------------------------------------------------------------------------

const ThemeContext = createContext<Accessor<FluentTokens>>(() => fluentDark)

export const ThemeProvider = ThemeContext.Provider

export function useTheme(): Accessor<FluentTokens> {
  return useContext(ThemeContext)
}
