import type { MotionProps } from "./app/motion/types.ts"

export interface WindowIntrinsicProps {
  title?: string
  visible?: boolean
  frameless?: boolean
  transparentBackground?: boolean
  alwaysOnTop?: boolean
  gpu?: boolean
  width?: number
  height?: number
  minWidth?: number
  minHeight?: number
  windowKind?: number
  screenX?: number
  screenY?: number
  enabled?: boolean
  onCloseRequested?: () => void
  onHoverEnter?: () => void
  onHoverLeave?: () => void
}

// ---------------------------------------------------------------------------
// Event props
// ---------------------------------------------------------------------------

export interface WheelEventPayload {
  deltaX: number
  deltaY: number
  angleDeltaX: number
  angleDeltaY: number
  pixelDeltaX: number
  pixelDeltaY: number
  x: number
  y: number
  phase: number
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
  metaKey: boolean
}

export interface EventProps {
  onClick?: (e: unknown) => void
  onDoubleClick?: (e: unknown) => void
  onPointerDown?: (e: unknown) => void
  onPointerUp?: (e: unknown) => void
  onPointerMove?: (e: unknown) => void
  onPointerEnter?: (e: unknown) => void
  onPointerLeave?: (e: unknown) => void
  onContextMenu?: (e: { x: number; y: number; screenX: number; screenY: number }) => void
  onKeyDown?: (e: unknown) => void
  onKeyUp?: (e: unknown) => void
  onWheel?: (e: WheelEventPayload) => void
  onFocusIn?: () => void
  onFocusOut?: () => void
  onLayout?: (e: { x: number; y: number; width: number; height: number }) => void
}

// ---------------------------------------------------------------------------
// Transform props (paint-level, not layout)
// ---------------------------------------------------------------------------

export interface TransformProps {
  transformX?: number
  transformY?: number
  scale?: number
  scaleX?: number
  scaleY?: number
  rotate?: number
  originX?: number
  originY?: number
  perspective?: number
}

// ---------------------------------------------------------------------------
// Visual props
// ---------------------------------------------------------------------------

export interface VisualProps {
  opacity?: number
  visible?: boolean
  blendMode?: "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten" | "color-dodge" | "color-burn" | "hard-light" | "soft-light" | "difference" | "exclusion" | "hue" | "saturation" | "color" | "luminosity"
  clip?: boolean
  clipPath?: string
  cursor?: number
  mask?: unknown
}

// ---------------------------------------------------------------------------
// Filter props
// ---------------------------------------------------------------------------

export interface FilterProps {
  backdropBlur?: number
  filterGrayscale?: number
  filterSaturate?: number
  filterBrightness?: number
  filterContrast?: number
  filterHueRotate?: number
  filterInvert?: number
  filterSepia?: number
  vibrancyDesaturation?: number
  vibrancyBlendMode?: number
}

// ---------------------------------------------------------------------------
// Layout props
// ---------------------------------------------------------------------------

export type SizingValue = 'hug' | 'fill' | number | `${number}%` | `${number}fr`

type AlignVertical = 'top' | 'center' | 'bottom' | 'stretch'
type AlignHorizontal = 'left' | 'center' | 'right' | 'stretch'
type AlignValue = AlignVertical | AlignHorizontal | `${AlignVertical} ${AlignHorizontal}`

export interface LayoutProps {
  // Direction
  row?: boolean
  column?: boolean
  // Sizing
  w?: SizingValue
  h?: SizingValue
  // Alignment & distribution
  align?: AlignValue
  spacing?: 'between' | 'around' | 'evenly'
  // Gap, padding, margin
  gap?: number
  padding?: number
  paddingTop?: number
  paddingRight?: number
  paddingBottom?: number
  paddingLeft?: number
  margin?: number
  marginTop?: number
  marginRight?: number
  marginBottom?: number
  marginLeft?: number
  // Constraints
  minWidth?: number
  maxWidth?: number
  minHeight?: number
  maxHeight?: number
  // Absolute positioning
  absolute?: boolean
  top?: number
  right?: number
  bottom?: number
  left?: number
  // Visibility (layout participation)
  layoutVisible?: boolean
  // Overflow & stacking
  overflow?: "visible" | "clip" | "hidden" | "scroll"
  overflowX?: "visible" | "clip" | "hidden" | "scroll"
  overflowY?: "visible" | "clip" | "hidden" | "scroll"
  zIndex?: number
  // Wrap
  wrap?: boolean
  // Grid child placement
  gridRow?: number
  gridColumn?: number
  rowSpan?: number
  colSpan?: number
}

// ---------------------------------------------------------------------------
// Common props — composed from sub-interfaces
// ---------------------------------------------------------------------------

export interface CommonProps extends EventProps, MotionProps, TransformProps, VisualProps, FilterProps, LayoutProps {
  ref?: (node: import("./runtime/fragment.ts").FragmentRendererNode) => void
  pointerEvents?: boolean
  focusable?: boolean
  children?: unknown
}

// ---------------------------------------------------------------------------
// Gradient brush types
// ---------------------------------------------------------------------------

export interface LinearGradientBrush {
  type: "linearGradient"
  startX: number
  startY: number
  endX: number
  endY: number
  stops: Array<{ offset: number; color: string }>
}

export interface RadialGradientBrush {
  type: "radialGradient"
  centerX: number
  centerY: number
  radius: number
  stops: Array<{ offset: number; color: string }>
}

export interface SweepGradientBrush {
  type: "sweepGradient"
  centerX: number
  centerY: number
  startAngle: number
  endAngle: number
  stops: Array<{ offset: number; color: string }>
}

export type GradientBrush = LinearGradientBrush | RadialGradientBrush | SweepGradientBrush

// ---------------------------------------------------------------------------
// Fragment kind-specific props
// ---------------------------------------------------------------------------

export interface GroupProps extends CommonProps {}

export interface RectProps extends CommonProps {
  cornerRadius?: number | { topLeft: number; topRight: number; bottomRight: number; bottomLeft: number }
  fill?: string | GradientBrush
  shadow?: { offsetX: number; offsetY: number; blur: number; color: string; inset?: boolean }
  stroke?: string
  strokeWidth?: number
  borderTop?: { width: number; color: string }
  borderRight?: { width: number; color: string }
  borderBottom?: { width: number; color: string }
  borderLeft?: { width: number; color: string }
}

export interface CircleProps extends CommonProps {
  cx?: number
  cy?: number
  r?: number
  fill?: string
  stroke?: string
  strokeWidth?: number
}

export interface TextProps extends CommonProps {
  text?: string
  fontSize?: number
  fontFamily?: string
  fontWeight?: number
  fontStyle?: string
  textMaxWidth?: number
  textOverflow?: "clip" | "ellipsis"
  color?: string
}

export interface TextInputProps extends CommonProps {
  text?: string
  fontSize?: number
  fontFamily?: string
  fontWeight?: number
  fontStyle?: string
  color?: string
  cursorPos?: number
  selectionAnchor?: number
  onTextChange?: (e: { text: string; cursor: number; selStart: number; selEnd: number }) => void
}

export interface PathProps extends CommonProps {
  d?: string
  fill?: string | GradientBrush
  stroke?: string
  strokeWidth?: number
}

export interface SpanProps extends CommonProps {
  text?: string
  fontSize?: number
  fontFamily?: string
  fontWeight?: number
  fontStyle?: string
  color?: string
}

export interface ImageProps extends CommonProps {
  objectFit?: "fill" | "contain" | "cover" | "none"
}

export interface GridProps extends CommonProps {
  columns?: Array<number | `${number}fr` | 'hug'>
  rows?: Array<number | `${number}fr` | 'hug'>
  columnGap?: number
  rowGap?: number
}

// ---------------------------------------------------------------------------
// Motion config — JS-only, not backed by napi
// ---------------------------------------------------------------------------

export interface MotionConfig {
  layerEnabled: boolean
  layoutEnabled: boolean
  hitTestEnabled: boolean
}


