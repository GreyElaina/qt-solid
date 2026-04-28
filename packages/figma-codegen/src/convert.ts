// ---------------------------------------------------------------------------
// Figma node → qt-solid canvas fragment JSX conversion
// ---------------------------------------------------------------------------

/** Accumulated imports for the generated file. */
export type ImportSet = Map<string, Set<string>>

export interface ConvertContext {
  imports: ImportSet
  indent: number
  /** "auto" = try component match first; "fragments" = always fragments */
  mode: "auto" | "fragments"
}

export function createContext(mode: "auto" | "fragments"): ConvertContext {
  return { imports: new Map(), indent: 0, mode }
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

function rgbaToHex(r: number, g: number, b: number, a: number): string {
  const toHex = (v: number) => Math.round(v * 255).toString(16).padStart(2, "0")
  if (a >= 0.999) return `#${toHex(r)}${toHex(g)}${toHex(b)}`
  return `#${toHex(r)}${toHex(g)}${toHex(b)}${toHex(a)}`
}

function paintToColor(paint: Paint): string | null {
  if (paint.type !== "SOLID" || paint.visible === false) return null
  const { r, g, b } = paint.color
  return rgbaToHex(r, g, b, paint.opacity ?? 1)
}

/** Linear gradient representation matching qt-solid's FragmentValue format. */
interface GradientFill {
  type: "lineargradient"
  angle: number
  stopOffsets: number[]
  stopColors: string[]
}

interface RadialGradientFill {
  type: "radialGradient"
  centerX: number
  centerY: number
  radius: number
  stops: Array<{ offset: number; color: string }>
}

interface SweepGradientFill {
  type: "sweepGradient"
  centerX: number
  centerY: number
  startAngle: number
  endAngle: number
  stops: Array<{ offset: number; color: string }>
}

class TokenRef {
  constructor(public expr: string) {}
}

type FillValue = string | GradientFill | RadialGradientFill | SweepGradientFill | TokenRef

function extractFill(fills: readonly Paint[] | typeof figma.mixed): FillValue | null {
  if (fills === figma.mixed || !Array.isArray(fills)) return null
  for (const p of fills) {
    if (p.visible === false) continue
    if (p.type === "SOLID") {
      return paintToColor(p)
    }
    if (p.type === "GRADIENT_LINEAR") {
      const stops = p.gradientStops
      // Figma gradient handles → angle. handlePositions[0] and [1] define the line.
      const h = p.gradientHandlePositions
      const angle = h && h.length >= 2
        ? Math.round(Math.atan2(h[1].y - h[0].y, h[1].x - h[0].x) * (180 / Math.PI))
        : 180
      return {
        type: "lineargradient",
        angle,
        stopOffsets: stops.map((s: ColorStop) => Math.round(s.position * 1000) / 1000),
        stopColors: stops.map((s: ColorStop) => rgbaToHex(s.color.r, s.color.g, s.color.b, s.color.a)),
      }
    }
    if (p.type === "GRADIENT_RADIAL") {
      const stops = p.gradientStops
      const h = p.gradientHandlePositions
      const cx = h && h.length >= 1 ? h[0].x : 0.5
      const cy = h && h.length >= 1 ? h[0].y : 0.5
      return {
        type: "radialGradient",
        centerX: cx,
        centerY: cy,
        radius: h && h.length >= 2
          ? Math.sqrt((h[1].x - h[0].x) ** 2 + (h[1].y - h[0].y) ** 2)
          : 0.5,
        stops: stops.map((s: ColorStop) => ({ offset: Math.round(s.position * 1000) / 1000, color: rgbaToHex(s.color.r, s.color.g, s.color.b, s.color.a) })),
      }
    }
    if (p.type === "GRADIENT_ANGULAR") {
      const stops = p.gradientStops
      const h = p.gradientHandlePositions
      const cx = h && h.length >= 1 ? h[0].x : 0.5
      const cy = h && h.length >= 1 ? h[0].y : 0.5
      return {
        type: "sweepGradient",
        centerX: cx,
        centerY: cy,
        startAngle: 0,
        endAngle: 360,
        stops: stops.map((s: ColorStop) => ({ offset: Math.round(s.position * 1000) / 1000, color: rgbaToHex(s.color.r, s.color.g, s.color.b, s.color.a) })),
      }
    }
  }
  return null
}

/** Compat: extract first solid fill only (for stroke fallback etc.) */
function firstSolidFill(fills: readonly Paint[] | typeof figma.mixed): string | null {
  const f = extractFill(fills)
  return typeof f === "string" ? f : null
}

function firstSolidStroke(strokes: readonly Paint[]): string | null {
  for (const p of strokes) {
    const c = paintToColor(p)
    if (c) return c
  }
  return null
}

// ---------------------------------------------------------------------------
// Variable token resolution (T1.1)
// ---------------------------------------------------------------------------

function sanitizeTokenName(name: string): string {
  // Strip private/emoji prefixes like "🔒/"
  let cleaned = name.replace(/^[^\x20-\x7E]+\//g, "")
  // Replace / with .
  cleaned = cleaned.replace(/\//g, ".")
  // Convert each segment from kebab-case to camelCase
  cleaned = cleaned.split(".").map(function(seg) {
    return seg.replace(/-([a-z0-9])/g, function(_m, c) { return c.toUpperCase() })
  }).join(".")
  return cleaned
}

async function resolveVariableToken(alias: { id: string }): Promise<TokenRef | null> {
  try {
    const variable = await figma.variables.getVariableByIdAsync(alias.id)
    if (variable) {
      return new TokenRef("theme()." + sanitizeTokenName(variable.name))
    }
  } catch (_e) {
    // variable resolution can fail
  }
  return null
}

async function resolveNodeFill(node: SceneNode): Promise<FillValue | null> {
  if ("boundVariables" in node) {
    const bv = (node as any).boundVariables
    if (bv && bv.fills && Array.isArray(bv.fills) && bv.fills.length > 0) {
      const token = await resolveVariableToken(bv.fills[0])
      if (token) return token
    }
  }
  if (!("fills" in node)) return null
  return extractFill((node as any).fills)
}

async function resolveNodeStroke(node: SceneNode): Promise<string | TokenRef | null> {
  if ("boundVariables" in node) {
    const bv = (node as any).boundVariables
    if (bv && bv.strokes && Array.isArray(bv.strokes) && bv.strokes.length > 0) {
      const token = await resolveVariableToken(bv.strokes[0])
      if (token) return token
    }
  }
  if (!("strokes" in node)) return null
  return firstSolidStroke((node as any).strokes)
}

async function resolveNodeTextColor(node: TextNode): Promise<string | TokenRef | null> {
  if ("boundVariables" in node) {
    const bv = (node as any).boundVariables
    if (bv && bv.fills && Array.isArray(bv.fills) && bv.fills.length > 0) {
      const token = await resolveVariableToken(bv.fills[0])
      if (token) return token
    }
  }
  return firstSolidFill(node.fills)
}

// ---------------------------------------------------------------------------
// Shadow / effects
// ---------------------------------------------------------------------------

interface ShadowProps {
  offsetX: number
  offsetY: number
  blur: number
  color: string
  inset?: boolean
}

function extractShadow(node: SceneNode): ShadowProps | null {
  if (!("effects" in node)) return null
  const effects = (node as FrameNode).effects
  for (const e of effects) {
    if ((e.type === "DROP_SHADOW" || e.type === "INNER_SHADOW") && e.visible !== false) {
      const { r, g, b } = e.color
      const a = e.color.a
      return {
        offsetX: e.offset.x,
        offsetY: e.offset.y,
        blur: e.radius,
        color: rgbaToHex(r, g, b, a),
        inset: e.type === "INNER_SHADOW" ? true : undefined,
      }
    }
  }
  return null
}

function extractBlur(node: SceneNode): number | null {
  if (!("effects" in node)) return null
  const effects = (node as FrameNode).effects
  for (const e of effects) {
    if (e.type === "LAYER_BLUR" && e.visible !== false) {
      return e.radius
    }
  }
  return null
}

function extractBackdropBlur(node: SceneNode): number | null {
  if (!("effects" in node)) return null
  const effects = (node as FrameNode).effects
  for (const e of effects) {
    if (e.type === "BACKGROUND_BLUR" && e.visible !== false) {
      return e.radius
    }
  }
  return null
}

function extractBlendMode(node: SceneNode): string | null {
  if (!("blendMode" in node)) return null
  const bm = (node as FrameNode).blendMode
  if (bm === "NORMAL" || bm === "PASS_THROUGH") return null
  const map: Record<string, string> = {
    MULTIPLY: "multiply", SCREEN: "screen", OVERLAY: "overlay",
    DARKEN: "darken", LIGHTEN: "lighten", COLOR_DODGE: "color-dodge",
    COLOR_BURN: "color-burn", HARD_LIGHT: "hard-light", SOFT_LIGHT: "soft-light",
    DIFFERENCE: "difference", EXCLUSION: "exclusion", HUE: "hue",
    SATURATION: "saturation", COLOR: "color", LUMINOSITY: "luminosity",
  }
  return map[bm] ?? null
}

// ---------------------------------------------------------------------------
// Opacity
// ---------------------------------------------------------------------------

function extractOpacity(node: SceneNode): number | null {
  if ("opacity" in node) {
    const op = (node as FrameNode).opacity
    if (op < 0.999) return Math.round(op * 100) / 100
  }
  return null
}

// ---------------------------------------------------------------------------
// Absolute positioning detection
// ---------------------------------------------------------------------------

interface AbsolutePosition {
  x: number
  y: number
}

function extractAbsolutePosition(node: SceneNode): AbsolutePosition | null {
  // A child is absolutely positioned when its parent uses auto-layout
  // but this child has layoutPositioning === "ABSOLUTE".
  if (!("layoutPositioning" in node)) return null
  if ((node as FrameNode).layoutPositioning !== "ABSOLUTE") return null
  return { x: Math.round(node.x), y: Math.round(node.y) }
}

// ---------------------------------------------------------------------------
// Layout mapping
// ---------------------------------------------------------------------------

function mapAutoLayout(node: FrameNode | ComponentNode | InstanceNode): Record<string, unknown> {
  const props: Record<string, unknown> = {}

  if (node.layoutMode === "HORIZONTAL") {
    props.flexDirection = "row"
  } else if (node.layoutMode === "VERTICAL") {
    props.flexDirection = "column"
  }

  if (node.layoutMode !== "NONE") {
    const primaryAlign = node.primaryAxisAlignItems
    if (primaryAlign === "CENTER") props.justifyContent = "center"
    else if (primaryAlign === "MAX") props.justifyContent = "flex-end"
    else if (primaryAlign === "SPACE_BETWEEN") props.justifyContent = "space-between"

    const counterAlign = node.counterAxisAlignItems
    if (counterAlign === "CENTER") props.alignItems = "center"
    else if (counterAlign === "MAX") props.alignItems = "flex-end"

    if (node.itemSpacing > 0) {
      props.gap = node.itemSpacing
    }

    // T1.2: layout wrap
    const wrap = (node as any).layoutWrap
    if (wrap === "WRAP") {
      props.flexWrap = "wrap"
    } else if (wrap === "WRAP_REVERSE") {
      props.flexWrap = "wrap-reverse"
    }
  }

  if (node.layoutSizingHorizontal === "FILL") {
    props.flexGrow = 1
  }

  if (node.layoutSizingHorizontal === "HUG" || node.layoutSizingVertical === "HUG") {
    props.flexShrink = 0
  }

  if (node.clipsContent) {
    const overflowDir = (node as any).overflowDirection
    if (overflowDir === "HORIZONTAL_SCROLLING" || overflowDir === "VERTICAL_SCROLLING" || overflowDir === "HORIZONTAL_AND_VERTICAL_SCROLLING") {
      props.overflow = "scroll"
    } else {
      props.overflow = "clip"
    }
  }

  return props
}

// ---------------------------------------------------------------------------
// Prop serialization
// ---------------------------------------------------------------------------

function serializePropValue(value: unknown): string {
  if (value instanceof TokenRef) return `{${value.expr}}`
  if (typeof value === "string") return `"${value}"`
  if (typeof value === "number") return `{${value}}`
  if (typeof value === "boolean") return value ? "" : `{false}`
  if (typeof value === "object" && value !== null) {
    // Gradient fill — emit as FragmentValue-compatible object literal
    if ("type" in value && (value as any).type === "lineargradient") {
      const g = value as GradientFill
      const offsets = `[${g.stopOffsets.join(", ")}]`
      const colors = `[${g.stopColors.map(c => `"${c}"`).join(", ")}]`
      return `{{ type: "lineargradient", angle: ${g.angle}, stopOffsets: ${offsets}, stopColors: ${colors} }}`
    }
    if ("type" in value && (value as any).type === "radialGradient") {
      const g = value as RadialGradientFill
      const stops = `[${g.stops.map(s => `{ offset: ${s.offset}, color: "${s.color}" }`).join(", ")}]`
      return `{{ type: "radialGradient", centerX: ${g.centerX}, centerY: ${g.centerY}, radius: ${g.radius}, stops: ${stops} }}`
    }
    if ("type" in value && (value as any).type === "sweepGradient") {
      const g = value as SweepGradientFill
      const stops = `[${g.stops.map(s => `{ offset: ${s.offset}, color: "${s.color}" }`).join(", ")}]`
      return `{{ type: "sweepGradient", centerX: ${g.centerX}, centerY: ${g.centerY}, startAngle: ${g.startAngle}, endAngle: ${g.endAngle}, stops: ${stops} }}`
    }
    // Array (e.g. bezier easing)
    if (Array.isArray(value)) {
      return `{[${value.map(function (v) { return typeof v === "string" ? '"' + v + '"' : v }).join(", ")}]}`
    }
    // Generic inline object (shadow, radii, motion targets, etc.)
    const entries = Object.entries(value)
      .map(function ([k, v]) {
        if (typeof v === "string") return k + ': "' + v + '"'
        if (Array.isArray(v)) return k + ": [" + v.join(", ") + "]"
        if (typeof v === "object" && v !== null) {
          var inner = Object.entries(v)
            .map(function ([ik, iv]) { return typeof iv === "string" ? ik + ': "' + iv + '"' : ik + ": " + iv })
            .join(", ")
          return k + ": { " + inner + " }"
        }
        return k + ": " + v
      })
      .join(", ")
    return `{{ ${entries} }}`
  }
  return `{${JSON.stringify(value)}}`
}

function serializeProps(props: Record<string, unknown>): string {
  const parts: string[] = []
  for (const [key, value] of Object.entries(props)) {
    if (value == null) continue
    if (typeof value === "boolean" && value) {
      parts.push(key)
    } else {
      parts.push(`${key}=${serializePropValue(value)}`)
    }
  }
  return parts.join(" ")
}

// ---------------------------------------------------------------------------
// Indent helper
// ---------------------------------------------------------------------------

function pad(ctx: ConvertContext): string {
  return "  ".repeat(ctx.indent)
}

// ---------------------------------------------------------------------------
// Shared: apply common visual props to a props dict
// ---------------------------------------------------------------------------

function applyVisualProps(props: Record<string, unknown>, node: SceneNode): void {
  // Shadow
  const shadow = extractShadow(node)
  if (shadow) {
    const shadowObj: Record<string, unknown> = { offsetX: shadow.offsetX, offsetY: shadow.offsetY, blur: shadow.blur, color: shadow.color }
    if (shadow.inset) shadowObj.inset = true
    props.shadow = shadowObj
  }

  // Blur (not supported on rect yet, but emit for future)
  const blur = extractBlur(node)
  if (blur) {
    props.blur = blur
  }

  // Backdrop blur
  const backdropBlur = extractBackdropBlur(node)
  if (backdropBlur) {
    props.backdropBlur = backdropBlur
  }

  // Blend mode
  const blendMode = extractBlendMode(node)
  if (blendMode) {
    props.blendMode = blendMode
  }

  // Opacity
  const opacity = extractOpacity(node)
  if (opacity != null) {
    props.opacity = opacity
  }

  // Absolute positioning
  const absPos = extractAbsolutePosition(node)
  if (absPos) {
    props.position = "absolute"
    props.x = absPos.x
    props.y = absPos.y
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function toCamelCase(str: string): string {
  return str
    .replace(/[^a-zA-Z0-9]+(.)/g, function (_m, ch) { return ch.toUpperCase() })
    .replace(/^[A-Z]/, function (ch) { return ch.toLowerCase() })
}

// ---------------------------------------------------------------------------
// Core conversion
// ---------------------------------------------------------------------------

import { matchComponent, resolveVariantProps, type ComponentMapping } from "./manifest.ts"
import { extractInteractions } from "./interactions.ts"

export async function convertNode(node: SceneNode, ctx: ConvertContext): Promise<string> {
  // Skip invisible nodes.
  if ("visible" in node && !node.visible) return ""

  // Component match (auto mode).
  if (ctx.mode === "auto" && node.type === "INSTANCE") {
    const mapping = await matchComponent(node)
    if (mapping) return convertComponentInstance(node as InstanceNode, mapping, ctx)
  }

  switch (node.type) {
    case "FRAME":
    case "COMPONENT":
    case "COMPONENT_SET":
    case "INSTANCE":
    case "SECTION":
      return convertFrame(node as FrameNode, ctx)
    case "GROUP":
      return convertGroup(node as GroupNode, ctx)
    case "RECTANGLE":
      return convertRectangle(node as RectangleNode, ctx)
    case "ELLIPSE":
      return convertEllipse(node as EllipseNode, ctx)
    case "TEXT":
      return convertText(node as TextNode, ctx)
    case "LINE":
      return convertLine(node as LineNode, ctx)
    case "VECTOR":
    case "BOOLEAN_OPERATION":
      return convertVector(node as VectorNode, ctx)
    default:
      return `${pad(ctx)}{/* unsupported: ${node.type} "${node.name}" */}`
  }
}

async function convertFrame(node: FrameNode, ctx: ConvertContext): Promise<string> {
  const props: Record<string, unknown> = {}

  // Layout
  Object.assign(props, mapAutoLayout(node))

  // Padding (per-side)
  const { paddingTop, paddingRight, paddingBottom, paddingLeft } = node
  if (paddingTop === paddingRight && paddingRight === paddingBottom && paddingBottom === paddingLeft) {
    if (paddingTop > 0) props.padding = paddingTop
  } else {
    if (paddingTop > 0) props.paddingTop = paddingTop
    if (paddingRight > 0) props.paddingRight = paddingRight
    if (paddingBottom > 0) props.paddingBottom = paddingBottom
    if (paddingLeft > 0) props.paddingLeft = paddingLeft
  }

  // Size
  if (node.layoutSizingHorizontal === "FIXED") props.width = Math.round(node.width)
  if (node.layoutSizingVertical === "FIXED") props.height = Math.round(node.height)
  if (node.minWidth != null && node.minWidth > 0) props.minWidth = node.minWidth
  if (node.minHeight != null && node.minHeight > 0) props.minHeight = node.minHeight
  if (node.maxWidth != null && node.maxWidth < Infinity) props.maxWidth = node.maxWidth
  if (node.maxHeight != null && node.maxHeight < Infinity) props.maxHeight = node.maxHeight

  // T1.3: Image fill → emit <image> element
  if (hasImageFill(node)) {
    const imgProps: Record<string, unknown> = {
      width: Math.round(node.width),
      height: Math.round(node.height),
    }
    applyVisualProps(imgProps, node)
    const fills = (node as any).fills
    if (Array.isArray(fills)) {
      for (const f of fills) {
        if (f.type === "IMAGE" && f.visible !== false) {
          const fitMap: Record<string, string> = { FILL: "cover", FIT: "contain", CROP: "cover", TILE: "none" }
          const objectFit = fitMap[f.scaleMode]
          if (objectFit && objectFit !== "cover") imgProps.objectFit = objectFit
          break
        }
      }
    }
    const propStr = serializeProps(imgProps)
    return pad(ctx) + "<image " + propStr + " />" + "{/* import image asset */}"
  }

  // Fill (solid or gradient) — T1.1: async variable resolution
  const fill = await resolveNodeFill(node)
  if (fill) props.fill = fill

  // Stroke — T1.1: async variable resolution
  const stroke = await resolveNodeStroke(node)
  if (stroke) {
    props.stroke = stroke
    if (node.strokeWeight !== figma.mixed && node.strokeWeight > 0) {
      props.strokeWidth = node.strokeWeight
    }
  }

  // Corner radius (uniform or per-corner)
  if ("cornerRadius" in node) {
    if (node.cornerRadius !== figma.mixed && node.cornerRadius > 0) {
      props.cornerRadius = node.cornerRadius
    } else if (node.cornerRadius === figma.mixed) {
      const tl = (node as any).topLeftRadius ?? 0
      const tr = (node as any).topRightRadius ?? 0
      const br = (node as any).bottomRightRadius ?? 0
      const bl = (node as any).bottomLeftRadius ?? 0
      if (tl > 0 || tr > 0 || br > 0 || bl > 0) {
        props.cornerRadius = { topLeft: tl, topRight: tr, bottomRight: br, bottomLeft: bl }
      }
    }
  }

  // Effects, opacity, absolute position
  applyVisualProps(props, node)

  // Phase B: interaction/animation mapping
  const interactions = await extractInteractions(node)
  if (interactions.whileHover) props.whileHover = interactions.whileHover
  if (interactions.whileTap) props.whileTap = interactions.whileTap
  if (interactions.whileFocus) props.whileFocus = interactions.whileFocus
  if (interactions.transition) props.transition = interactions.transition
  if (interactions.focusable) props.focusable = true
  if (interactions.cursor) props.cursor = interactions.cursor

  // Determine tag: use <rect> if has visual properties, <group> otherwise.
  const hasVisual = fill || stroke || props.cornerRadius || props.shadow
  const tag = hasVisual ? "rect" : "group"

  const children = "children" in node ? (node.children as readonly SceneNode[]) : []

  if (children.length === 0) {
    const propStr = serializeProps(props)
    return `${pad(ctx)}<${tag}${propStr ? " " + propStr : ""} />`
  }

  const propStr = serializeProps(props)
  const lines = [`${pad(ctx)}<${tag}${propStr ? " " + propStr : ""}>`]
  ctx.indent++
  for (const child of children) {
    const line = await convertNode(child, ctx)
    if (line) lines.push(line)
  }
  ctx.indent--
  lines.push(`${pad(ctx)}</${tag}>`)
  return lines.join("\n")
}

async function convertGroup(node: GroupNode, ctx: ConvertContext): Promise<string> {
  const children = node.children as readonly SceneNode[]
  if (children.length === 0) return ""

  if (children.length === 1) {
    return convertNode(children[0], ctx)
  }

  const props: Record<string, unknown> = {}
  applyVisualProps(props, node)

  const propStr = serializeProps(props)
  const lines = [`${pad(ctx)}<group${propStr ? " " + propStr : ""}>`]
  ctx.indent++
  for (const child of children) {
    const line = await convertNode(child, ctx)
    if (line) lines.push(line)
  }
  ctx.indent--
  lines.push(`${pad(ctx)}</group>`)
  return lines.join("\n")
}

async function convertRectangle(node: RectangleNode, ctx: ConvertContext): Promise<string> {
  if (hasImageFill(node)) {
    const imgProps: Record<string, unknown> = {
      width: Math.round(node.width),
      height: Math.round(node.height),
    }
    applyVisualProps(imgProps, node)
    const fills = (node as any).fills
    if (Array.isArray(fills)) {
      for (const f of fills) {
        if (f.type === "IMAGE" && f.visible !== false) {
          const fitMap: Record<string, string> = { FILL: "cover", FIT: "contain", CROP: "cover", TILE: "none" }
          const objectFit = fitMap[f.scaleMode]
          if (objectFit && objectFit !== "cover") imgProps.objectFit = objectFit
          break
        }
      }
    }
    return pad(ctx) + "<image " + serializeProps(imgProps) + " />" + "{/* import image asset */}"
  }

  const props: Record<string, unknown> = {}
  props.width = Math.round(node.width)
  props.height = Math.round(node.height)

  const fill = await resolveNodeFill(node)
  if (fill) props.fill = fill

  const stroke = await resolveNodeStroke(node)
  if (stroke) {
    props.stroke = stroke
    if (node.strokeWeight !== figma.mixed && node.strokeWeight > 0) {
      props.strokeWidth = node.strokeWeight
    }
  }

  if (node.cornerRadius !== figma.mixed && node.cornerRadius > 0) {
    props.cornerRadius = node.cornerRadius
  } else if (node.cornerRadius === figma.mixed) {
    const tl = ((node as any).topLeftRadius != null ? (node as any).topLeftRadius : 0)
    const tr = ((node as any).topRightRadius != null ? (node as any).topRightRadius : 0)
    const br = ((node as any).bottomRightRadius != null ? (node as any).bottomRightRadius : 0)
    const bl = ((node as any).bottomLeftRadius != null ? (node as any).bottomLeftRadius : 0)
    if (tl > 0 || tr > 0 || br > 0 || bl > 0) {
      props.cornerRadius = { topLeft: tl, topRight: tr, bottomRight: br, bottomLeft: bl }
    }
  }

  applyVisualProps(props, node)
  return `${pad(ctx)}<rect ${serializeProps(props)} />`
}

async function convertEllipse(node: EllipseNode, ctx: ConvertContext): Promise<string> {
  const props: Record<string, unknown> = {}
  const r = Math.round(Math.min(node.width, node.height) / 2)
  props.r = r

  const fill = await resolveNodeFill(node)
  if (fill) props.fill = fill

  const stroke = await resolveNodeStroke(node)
  if (stroke) {
    props.stroke = stroke
    if (node.strokeWeight !== figma.mixed && node.strokeWeight > 0) {
      props.strokeWidth = node.strokeWeight
    }
  }

  applyVisualProps(props, node)
  return `${pad(ctx)}<circle ${serializeProps(props)} />`
}

async function convertText(node: TextNode, ctx: ConvertContext): Promise<string> {
  // Check for mixed styles — if so, emit <text> with <span> children
  const hasMixedStyle = node.fontSize === figma.mixed
    || node.fontName === figma.mixed
    || node.fontWeight === figma.mixed

  if (hasMixedStyle) {
    const segments = node.getStyledTextSegments(["fontSize", "fontName", "fontWeight", "fills"])
    if (segments.length > 1) {
      const textProps: Record<string, unknown> = {}
      if ("layoutSizingHorizontal" in node && (node as any).layoutSizingHorizontal === "FILL") {
        textProps.flexGrow = 1
      }
      applyVisualProps(textProps, node)
      const propStr = serializeProps(textProps)
      const lines = [`${pad(ctx)}<text${propStr ? " " + propStr : ""}>`]
      ctx.indent++
      for (const seg of segments) {
        const spanProps: Record<string, unknown> = { text: seg.characters }
        if (seg.fontSize) spanProps.fontSize = seg.fontSize
        if (seg.fontName && seg.fontName.family) spanProps.fontFamily = seg.fontName.family
        if (seg.fontWeight) spanProps.fontWeight = seg.fontWeight
        // Segment fill color
        if (seg.fills && Array.isArray(seg.fills)) {
          for (const f of seg.fills) {
            if (f.type === "SOLID" && f.visible !== false) {
              const { r, g, b } = f.color
              const toHex = (v: number) => Math.round(v * 255).toString(16).padStart(2, "0")
              spanProps.color = `#${toHex(r)}${toHex(g)}${toHex(b)}`
              break
            }
          }
        }
        lines.push(`${pad(ctx)}<span ${serializeProps(spanProps)} />`)
      }
      ctx.indent--
      lines.push(`${pad(ctx)}</text>`)
      return lines.join("\n")
    }
  }

  // Uniform style — original path
  const props: Record<string, unknown> = {}
  props.text = node.characters

  if (node.fontSize !== figma.mixed) {
    props.fontSize = node.fontSize
  }

  if (node.fontName !== figma.mixed && node.fontName.family) {
    props.fontFamily = node.fontName.family
  }

  if (node.fontWeight !== figma.mixed) {
    props.fontWeight = node.fontWeight
  }

  const fill = await resolveNodeTextColor(node)
  if (fill) props.color = fill

  if ("layoutSizingHorizontal" in node && (node as any).layoutSizingHorizontal === "FILL") {
    props.flexGrow = 1
  }

  applyVisualProps(props, node)
  return `${pad(ctx)}<text ${serializeProps(props)} />`
}

async function convertLine(node: LineNode, ctx: ConvertContext): Promise<string> {
  const props: Record<string, unknown> = { height: 1, flexGrow: 1 }
  const stroke = await resolveNodeStroke(node)
  if (stroke) props.fill = stroke
  else props.fill = "#FFFFFF14"
  applyVisualProps(props, node)
  return `${pad(ctx)}<rect ${serializeProps(props)} />`
}

async function convertVector(node: VectorNode, ctx: ConvertContext): Promise<string> {
  const props: Record<string, unknown> = {}

  const fill = await resolveNodeFill(node)
  if (fill) props.fill = fill

  const stroke = await resolveNodeStroke(node)
  if (stroke) {
    props.stroke = stroke
    if (node.strokeWeight !== figma.mixed && node.strokeWeight > 0) {
      props.strokeWidth = node.strokeWeight
    }
  }

  applyVisualProps(props, node)

  // Export SVG and extract path d attribute(s).
  let pathData = ""
  try {
    const svgBytes = await node.exportAsync({ format: "SVG" })
    const svgStr = String.fromCharCode(...svgBytes)
    // Extract all d="..." from the SVG.
    const dMatches = [...svgStr.matchAll(/\bd="([^"]+)"/g)]
    if (dMatches.length === 1) {
      pathData = dMatches[0][1]
    } else if (dMatches.length > 1) {
      // Multiple paths — join them.
      pathData = dMatches.map(m => m[1]).join(" ")
    }
  } catch (_e) {
    // exportAsync can fail for detached nodes, etc.
  }

  if (pathData) {
    props.d = pathData
    const propStr = serializeProps(props)
    return `${pad(ctx)}<path ${propStr} />`
  }

  const propStr = serializeProps(props)
  return `${pad(ctx)}<path d=""${propStr ? " " + propStr : ""} />{/* could not extract path for "${node.name}" */}`
}

// ---------------------------------------------------------------------------
// Image node (Figma doesn't have a dedicated IMAGE type, but rectangles
// with image fills act as images)
// ---------------------------------------------------------------------------

function hasImageFill(node: SceneNode): boolean {
  if (!("fills" in node)) return false
  const fills = (node as RectangleNode).fills
  if (fills === figma.mixed || !Array.isArray(fills)) return false
  return fills.some(f => f.type === "IMAGE" && f.visible !== false)
}

// ---------------------------------------------------------------------------
// Component instance conversion
// ---------------------------------------------------------------------------

async function convertComponentInstance(
  node: InstanceNode,
  mapping: ComponentMapping,
  ctx: ConvertContext,
): Promise<string> {
  // Track import.
  if (!ctx.imports.has(mapping.from)) {
    ctx.imports.set(mapping.from, new Set())
  }
  ctx.imports.get(mapping.from)!.add(mapping.name)

  const props = resolveVariantProps(node, mapping)

  // Variant properties → pass as props (camelCased)
  const variantProperties = node.variantProperties
  if (variantProperties && !mapping.variantProps) {
    for (const [key, value] of Object.entries(variantProperties)) {
      const camelKey = toCamelCase(key)
      props[camelKey] = value
    }
  }

  // T1.4: Read component properties (boolean + text)
  const compProps = node.componentProperties
  if (compProps) {
    for (const key of Object.keys(compProps)) {
      const prop = compProps[key]
      if (!prop) continue
      const camelKey = toCamelCase(key.replace(/#\d+:\d+$/, ""))
      if (prop.type === "BOOLEAN") {
        if (prop.value === true) {
          props[camelKey] = true
        } else if (prop.value === false) {
          props[camelKey] = false
        }
      } else if (prop.type === "TEXT") {
        if (typeof prop.value === "string" && prop.value.length > 0) {
          props[camelKey] = prop.value
        }
      }
    }
  }

  // Apply common visual overrides (opacity, shadow, absolute pos)
  applyVisualProps(props, node)

  // Children
  if (mapping.children === "text") {
    const textNode = findFirstText(node)
    if (textNode) {
      const propStr = serializeProps(props)
      return `${pad(ctx)}<${mapping.name}${propStr ? " " + propStr : ""}>${textNode.characters}</${mapping.name}>`
    }
  }

  if (mapping.children === "slot") {
    const children = node.children as readonly SceneNode[]
    if (children.length > 0) {
      const propStr = serializeProps(props)
      const lines = [`${pad(ctx)}<${mapping.name}${propStr ? " " + propStr : ""}>`]
      ctx.indent++
      for (const child of children) {
        const line = await convertNode(child, ctx)
        if (line) lines.push(line)
      }
      ctx.indent--
      lines.push(`${pad(ctx)}</${mapping.name}>`)
      return lines.join("\n")
    }
  }

  const propStr = serializeProps(props)
  return `${pad(ctx)}<${mapping.name}${propStr ? " " + propStr : ""} />`
}

function findFirstText(node: SceneNode): TextNode | null {
  if (node.type === "TEXT") return node
  if ("children" in node) {
    for (const child of (node as FrameNode).children) {
      const found = findFirstText(child)
      if (found) return found
    }
  }
  return null
}

// ---------------------------------------------------------------------------
// Import statement generation
// ---------------------------------------------------------------------------

export function generateImports(imports: ImportSet): string {
  const lines: string[] = []
  for (const [from, names] of imports) {
    const sorted = [...names].sort()
    lines.push(`import { ${sorted.join(", ")} } from "${from}"`)
  }
  return lines.join("\n")
}
