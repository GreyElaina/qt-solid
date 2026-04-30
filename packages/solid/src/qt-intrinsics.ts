// Inline intrinsic prop types — replaces deleted @qt-solid/core-widgets/qt-intrinsics.
// Only Window is a real widget now; other types retained for component-layer Props aliases.

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

// Retained for component-layer type aliases (ViewProps, LabelProps, etc.)
// These widgets no longer exist as native intrinsics — they are fragment-based.

export interface ViewIntrinsicProps {
  direction?: "column" | "row"
  justifyContent?: "flex-start" | "center" | "flex-end"
  alignItems?: "flex-start" | "center" | "flex-end" | "stretch"
  gap?: number
  padding?: number
  wrap?: "nowrap" | "wrap" | "wrap-reverse"
  backgroundColor?: string
  width?: number
  height?: number
  minWidth?: number
  minHeight?: number
  maxWidth?: number
  maxHeight?: number
  aspectRatio?: number
  grow?: number
  shrink?: number
  basis?: number
  alignSelf?: "auto" | "flex-start" | "flex-end" | "center" | "stretch"
  margin?: number
  enabled?: boolean
  hidden?: boolean
}

export interface TextIntrinsicProps {
  text?: string
  family?: string
  pointSize?: number
  weight?: string
  italic?: boolean
}

export interface LabelIntrinsicProps {
  text?: string
}

export interface ButtonIntrinsicProps {
  text?: string
  onClicked?: () => void
}

export interface InputIntrinsicProps {
  text?: string
  placeholder?: string
  focusPolicy?: string
  autoFocus?: boolean
  cursorPosition?: number
  selectionStart?: number
  selectionEnd?: number
  onChanged?: (text: string) => void
  onTextChanged?: (text: string) => void
  onCursorPositionChanged?: (pos: number) => void
  onSelectionChanged?: () => void
  onFocusIn?: () => void
  onFocusOut?: () => void
}

export interface CheckIntrinsicProps {
  text?: string
  checked?: boolean
  onToggled?: (checked: boolean) => void
}

export interface SliderIntrinsicProps {
  value?: number
  minimum?: number
  maximum?: number
  step?: number
  pageStep?: number
  onValueChanged?: (value: number) => void
  onFocusIn?: () => void
  onFocusOut?: () => void
}

export interface DoubleSpinBoxIntrinsicProps {
  value?: number
  minimum?: number
  maximum?: number
  step?: number
  onValueChanged?: (value: number) => void
  onFocusIn?: () => void
  onFocusOut?: () => void
}

export interface GroupIntrinsicProps {
  title?: string
}

export interface GridIntrinsicProps {
  rowGap?: number
  colGap?: number
}

export interface GridItemIntrinsicProps {
  gridRow?: number
  gridCol?: number
  gridRowSpan?: number
  gridColSpan?: number
  direction?: "column" | "row"
  justifyContent?: "flex-start" | "center" | "flex-end"
  alignItems?: "flex-start" | "center" | "flex-end" | "stretch"
  gap?: number
  padding?: number
}

// ---------------------------------------------------------------------------
// Canvas fragment common props (shared by all fragment kinds)
// ---------------------------------------------------------------------------

export interface WheelEventPayload {
  /** Scroll-offset delta: +X = increase scrollX (scroll right). Negated from raw Qt content delta. */
  deltaX: number
  /** Scroll-offset delta: +Y = increase scrollY (reveal content below). Negated from raw Qt content delta. */
  deltaY: number
  /** Raw angle delta (1/8 degree units from QWheelEvent::angleDelta). */
  angleDeltaX: number
  angleDeltaY: number
  /** High-precision pixel delta from trackpad (0 when unavailable). */
  pixelDeltaX: number
  pixelDeltaY: number
  x: number
  y: number
  /** Scroll phase: 0=none, 1=begin, 2=update, 3=end, 4=momentum. */
  phase: number
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
  metaKey: boolean
}

export interface CanvasEventProps {
  onClick?: (e: unknown) => void
  onDoubleClick?: (e: unknown) => void
  onPointerDown?: (e: unknown) => void
  onPointerUp?: (e: unknown) => void
  onPointerMove?: (e: unknown) => void
  onPointerEnter?: (e: unknown) => void
  onPointerLeave?: (e: unknown) => void
  onKeyDown?: (e: unknown) => void
  onKeyUp?: (e: unknown) => void
  onWheel?: (e: WheelEventPayload) => void
  onFocusIn?: () => void
  onFocusOut?: () => void
  onLayout?: (e: { x: number; y: number; width: number; height: number }) => void
}

export interface CanvasNodeHandle {
  readonly canvasNodeId: number
  readonly fragmentId: number
}

export interface CanvasCommonProps extends CanvasEventProps {
  ref?: (node: CanvasNodeHandle) => void
  x?: number
  y?: number
  opacity?: number
  backdropBlur?: number
  clip?: boolean
  visible?: boolean
  pointerEvents?: boolean
  cursor?: number
  focusable?: boolean
  blendMode?: "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten" | "color-dodge" | "color-burn" | "hard-light" | "soft-light" | "difference" | "exclusion" | "hue" | "saturation" | "color" | "luminosity"
  children?: unknown
  // Taffy layout props (accepted on all fragment nodes)
  width?: number | `${number}%` | "auto"
  height?: number | `${number}%` | "auto"
  flexDirection?: string
  flexGrow?: number
  flexShrink?: number
  flexBasis?: number
  flexWrap?: string
  alignItems?: string
  alignSelf?: string
  justifyContent?: string
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
  minWidth?: number
  minHeight?: number
  maxWidth?: number
  maxHeight?: number
  position?: string
  overflow?: "visible" | "clip" | "hidden" | "scroll"
  overflowX?: "visible" | "clip" | "hidden" | "scroll"
  overflowY?: "visible" | "clip" | "hidden" | "scroll"
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
// Canvas fragment kind-specific props
// ---------------------------------------------------------------------------

export interface CanvasGroupProps extends CanvasCommonProps {}

export interface CanvasRectProps extends CanvasCommonProps {
  cornerRadius?: number | { topLeft: number; topRight: number; bottomRight: number; bottomLeft: number }
  fill?: string | GradientBrush
  shadow?: { offsetX: number; offsetY: number; blur: number; color: string; inset?: boolean }
  stroke?: string
  strokeWidth?: number
}

export interface CanvasCircleProps extends CanvasCommonProps {
  cx?: number
  cy?: number
  r?: number
  fill?: string
  stroke?: string
  strokeWidth?: number
}

export interface CanvasTextProps extends CanvasCommonProps {
  text?: string
  fontSize?: number
  fontFamily?: string
  fontWeight?: number
  fontStyle?: string
  textMaxWidth?: number
  textOverflow?: "clip" | "ellipsis"
  color?: string
}

export interface CanvasTextInputProps extends CanvasCommonProps {
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

export interface CanvasPathProps extends CanvasCommonProps {
  d?: string
  fill?: string | GradientBrush
  stroke?: string
  strokeWidth?: number
}

export interface CanvasSpanProps extends CanvasCommonProps {
  text?: string
  fontSize?: number
  fontFamily?: string
  fontWeight?: number
  fontStyle?: string
  color?: string
}

export interface CanvasImageProps extends CanvasCommonProps {
  objectFit?: "fill" | "contain" | "cover" | "none"
}

// ---------------------------------------------------------------------------
// Motion config — JS-only, not backed by napi
// ---------------------------------------------------------------------------

export interface QtMotionConfig {
  layerEnabled: boolean
  layoutEnabled: boolean
  hitTestEnabled: boolean
}

// ---------------------------------------------------------------------------
// Intrinsic elements registry
// ---------------------------------------------------------------------------

export interface QtIntrinsicElements {
  window: WindowIntrinsicProps
  group: CanvasGroupProps
  rect: CanvasRectProps
  circle: CanvasCircleProps
  text: CanvasTextProps
  textinput: CanvasTextInputProps
  path: CanvasPathProps
  image: CanvasImageProps
  span: CanvasSpanProps
}
