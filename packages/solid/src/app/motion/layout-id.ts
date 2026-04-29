import {
  canvasFragmentGetWorldBounds,
  canvasFragmentSetLayoutFlip,
  type QtTransitionSpec,
} from "@qt-solid/core/native";
import type { TransitionSpec } from "./types.ts";

// ---------------------------------------------------------------------------
// Shared layout animation registry (layoutId FLIP)
// ---------------------------------------------------------------------------

interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface LayoutNode {
  canvasNodeId: number;
  fragmentId: number;
}

interface LayoutEntry {
  active?: LayoutNode;
  snapshot?: Bounds;
}

const registry = new Map<string, LayoutEntry>();

function registryKey(canvasNodeId: number, layoutId: string): string {
  return `${canvasNodeId}:${layoutId}`;
}

function measure(node: LayoutNode): Bounds | null {
  return canvasFragmentGetWorldBounds(node.canvasNodeId, node.fragmentId) ?? null;
}

function sameNode(a: LayoutNode | undefined, b: LayoutNode): boolean {
  return a != null && a.canvasNodeId === b.canvasNodeId && a.fragmentId === b.fragmentId;
}

// -- Lower TransitionSpec to native QtTransitionSpec --

const DEFAULT_LAYOUT_TRANSITION = {
  type: "spring",
  stiffness: 500,
  damping: 30,
  mass: 1,
} as QtTransitionSpec;

function lowerLayoutTransition(spec: TransitionSpec | undefined): QtTransitionSpec {
  if (!spec) return DEFAULT_LAYOUT_TRANSITION;

  const hasSpringFields = spec.stiffness != null
    || spec.damping != null
    || spec.mass != null
    || spec.velocity != null;
  const hasTweenFields = spec.duration != null || spec.ease != null;
  const type = (spec.type === "instant"
    ? "instant"
    : spec.type === "tween"
      ? "tween"
      : hasSpringFields
        ? "spring"
        : hasTweenFields
          ? "tween"
          : "spring") as QtTransitionSpec["type"];

  if (type === "spring") {
    return {
      type,
      stiffness: spec.stiffness ?? DEFAULT_LAYOUT_TRANSITION.stiffness,
      damping: spec.damping ?? DEFAULT_LAYOUT_TRANSITION.damping,
      mass: spec.mass ?? DEFAULT_LAYOUT_TRANSITION.mass,
      velocity: spec.velocity,
      restDelta: spec.restDelta,
      restSpeed: spec.restSpeed,
    };
  }

  if (type === "tween") {
    const ease = Array.isArray(spec.ease)
      ? spec.ease
      : undefined;
    return { type, duration: spec.duration ?? 0.3, ease };
  }

  return { type: "instant" } as QtTransitionSpec;
}

// -- Public API --

export function setLayoutId(
  node: LayoutNode,
  layoutId: string,
  transition: TransitionSpec | undefined,
): void {
  const key = registryKey(node.canvasNodeId, layoutId);
  const newBounds = measure(node);

  if (!newBounds) {
    // Fragment not yet laid out — register and wait
    registry.set(key, { active: node });
    return;
  }

  const entry = registry.get(key);
  let fromBounds = entry?.snapshot;

  // If no snapshot but an active predecessor exists, measure it directly
  if (!fromBounds && entry?.active && !sameNode(entry.active, node)) {
    fromBounds = measure(entry.active) ?? undefined;
  }

  registry.set(key, { active: node });

  if (fromBounds) {
    startFlip(node, fromBounds, newBounds, transition);
  }
}

export function unsetLayoutId(node: LayoutNode, layoutId: string): void {
  const key = registryKey(node.canvasNodeId, layoutId);
  const entry = registry.get(key);

  // Only snapshot if this node is still the active owner
  if (!entry || !sameNode(entry.active, node)) return;

  const snapshot = measure(node);
  if (snapshot) {
    registry.set(key, { snapshot });
  } else {
    registry.delete(key);
  }
}

// -- FLIP execution --

function startFlip(
  node: LayoutNode,
  from: Bounds,
  to: Bounds,
  transition: TransitionSpec | undefined,
): void {
  const dx = from.x - to.x;
  const dy = from.y - to.y;
  const sx = to.width > 0 ? from.width / to.width : 1;
  const sy = to.height > 0 ? from.height / to.height : 1;

  // Skip if delta is negligible
  if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5
    && Math.abs(sx - 1) < 0.001 && Math.abs(sy - 1) < 0.001) {
    return;
  }

  const spec = lowerLayoutTransition(transition);
  canvasFragmentSetLayoutFlip(
    node.canvasNodeId,
    node.fragmentId,
    dx, dy, sx, sy,
    spec,
  );
}
