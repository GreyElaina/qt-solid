// ---------------------------------------------------------------------------
// Phase B: Figma reaction/interaction → qt-solid motion props
// ---------------------------------------------------------------------------

export interface MotionTarget {
  opacity?: number
  backgroundColor?: string
  borderRadius?: number
  scale?: number
  scaleX?: number
  scaleY?: number
  rotate?: number
  blur?: number
}

export interface TransitionSpec {
  type?: "tween" | "spring" | "instant"
  duration?: number
  ease?: string | [number, number, number, number]
  stiffness?: number
  damping?: number
  mass?: number
}

export interface InteractionProps {
  whileHover?: MotionTarget
  whileTap?: MotionTarget
  whileFocus?: MotionTarget
  transition?: TransitionSpec
  focusable?: boolean
  cursor?: string
}

// ---------------------------------------------------------------------------
// Visual diffing
// ---------------------------------------------------------------------------

function getNodeOpacity(node: SceneNode): number {
  return "opacity" in node ? (node as FrameNode).opacity : 1
}

function getNodeFillColor(node: SceneNode): string | null {
  if (!("fills" in node)) return null
  const fills = (node as FrameNode).fills
  if (fills === figma.mixed || !Array.isArray(fills)) return null
  for (const f of fills) {
    if (f.type === "SOLID" && f.visible !== false) {
      const toHex = (v: number) => Math.round(v * 255).toString(16).padStart(2, "0")
      return `#${toHex(f.color.r)}${toHex(f.color.g)}${toHex(f.color.b)}`
    }
  }
  return null
}

function getNodeCornerRadius(node: SceneNode): number | null {
  if (!("cornerRadius" in node)) return null
  const cr = (node as FrameNode).cornerRadius
  if (cr === figma.mixed) return null
  return cr > 0 ? cr : null
}

function getNodeBlur(node: SceneNode): number | null {
  if (!("effects" in node)) return null
  const effects = (node as FrameNode).effects
  for (const e of effects) {
    if (e.type === "LAYER_BLUR" && e.visible !== false) {
      return e.radius
    }
  }
  return null
}

export function diffVisualTarget(from: SceneNode, to: SceneNode): MotionTarget {
  const target: MotionTarget = {}

  const fromOp = getNodeOpacity(from)
  const toOp = getNodeOpacity(to)
  if (Math.abs(fromOp - toOp) > 0.001) {
    target.opacity = Math.round(toOp * 100) / 100
  }

  const fromColor = getNodeFillColor(from)
  const toColor = getNodeFillColor(to)
  if (toColor && fromColor !== toColor) {
    target.backgroundColor = toColor
  }

  const fromRadius = getNodeCornerRadius(from)
  const toRadius = getNodeCornerRadius(to)
  if (toRadius != null && fromRadius !== toRadius) {
    target.borderRadius = toRadius
  }

  if (from.width > 0 && from.height > 0) {
    var sx = to.width / from.width
    var sy = to.height / from.height
    // Only emit scale for reasonable ratios — large ratios mean
    // we're comparing different-purpose frames, not a scale animation.
    if (sx > 0.5 && sx < 2 && sy > 0.5 && sy < 2) {
      if (Math.abs(sx - 1) > 0.01 || Math.abs(sy - 1) > 0.01) {
        if (Math.abs(sx - sy) < 0.01) {
          target.scale = Math.round(sx * 1000) / 1000
        } else {
          target.scaleX = Math.round(sx * 1000) / 1000
          target.scaleY = Math.round(sy * 1000) / 1000
        }
      }
    }
  }

  // Rotation diff
  if ("rotation" in from && "rotation" in to) {
    const fromRot = (from as FrameNode).rotation ?? 0
    const toRot = (to as FrameNode).rotation ?? 0
    if (Math.abs(fromRot - toRot) > 0.1) {
      target.rotate = Math.round(toRot * 10) / 10
    }
  }

  // Blur diff (layer blur)
  const fromBlur = getNodeBlur(from)
  const toBlur = getNodeBlur(to)
  if (fromBlur !== toBlur && toBlur != null) {
    target.blur = toBlur
  }

  return target
}

// ---------------------------------------------------------------------------
// Transition mapping
// ---------------------------------------------------------------------------

const EASING_MAP: Record<string, string> = {
  LINEAR: "linear",
  EASE_IN: "ease-in",
  EASE_OUT: "ease-out",
  EASE_IN_AND_OUT: "ease-in-out",
}

export function mapTransition(transition: any): TransitionSpec | null {
  if (!transition) return null

  const spec: TransitionSpec = {}

  const ttype = transition.type
  if (ttype === "SPRING") {
    spec.type = "spring"
  } else {
    spec.type = "tween"
  }

  if (transition.duration != null && transition.duration > 0) {
    spec.duration = transition.duration
  }

  const easing = transition.easing
  if (easing) {
    const named = EASING_MAP[easing.type]
    if (named) {
      spec.ease = named
    } else if (easing.type === "CUSTOM_BEZIER" || easing.type === "CUSTOM_CUBIC_BEZIER") {
      const fn = easing.easingFunctionCubicBezier
      if (fn) {
        spec.ease = [fn.x1, fn.y1, fn.x2, fn.y2]
      }
    }
  }

  return spec
}

// ---------------------------------------------------------------------------
// Main extraction
// ---------------------------------------------------------------------------

export async function extractInteractions(node: SceneNode): Promise<InteractionProps> {
  const result: InteractionProps = {}

  if (!("reactions" in node)) return result
  const reactions = (node as FrameNode).reactions
  if (!reactions || reactions.length === 0) return result

  for (const reaction of reactions) {
    const trigger = reaction.trigger
    if (!trigger) continue

    let actions = reaction.actions
    if (!actions && (reaction as any).action) {
      actions = [(reaction as any).action]
    }
    if (!actions) continue

    for (const action of actions) {
      if (!action || action.type !== "NODE" || !action.destinationId) continue

      let destNode: BaseNode | null = null
      try {
        destNode = await figma.getNodeByIdAsync(action.destinationId)
      } catch (_e) {
        continue
      }
      if (!destNode || !("width" in destNode)) continue

      const motionTarget = diffVisualTarget(node, destNode as SceneNode)
      if (Object.keys(motionTarget).length === 0) continue

      const triggerType = trigger.type

      if (triggerType === "ON_HOVER" || triggerType === "MOUSE_ENTER") {
        result.whileHover = motionTarget
        const t = mapTransition(action.transition)
        if (t && !result.transition) result.transition = t
      }

      if (triggerType === "ON_CLICK" || triggerType === "ON_PRESS") {
        result.whileTap = motionTarget
        const t = mapTransition(action.transition)
        if (t && !result.transition) result.transition = t
      }

      if ((triggerType as string) === "ON_FOCUS") {
        result.whileFocus = motionTarget
        const t = mapTransition(action.transition)
        if (t && !result.transition) result.transition = t
      }
    }
  }

  // Infer focusable from trigger types
  for (const reaction of reactions) {
    const trigger = reaction.trigger
    if (!trigger) continue
    const tt = trigger.type as string
    if (tt === "ON_FOCUS" || tt === "ON_KEY_DOWN") {
      result.focusable = true
      break
    }
  }

  // Infer cursor from click/press reactions
  for (const reaction of reactions) {
    const trigger = reaction.trigger
    if (!trigger) continue
    if (trigger.type === "ON_CLICK" || trigger.type === "ON_PRESS") {
      result.cursor = "pointer"
      break
    }
  }

  return result
}
