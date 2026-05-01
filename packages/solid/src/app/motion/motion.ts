import {
  createEffect,
  createMemo,
  createSignal,
  on,
  onCleanup,
  onMount,
  splitProps as solidSplitProps,
  untrack,
  type Accessor,
  type Component,
  type JSX,
} from "solid-js";
import {
  canvasFragmentSetLayoutFlip,
  canvasFragmentGetWorldBounds,
  canvasFragmentSetListener,
  type QtMotionTarget,
  type QtNode,
  type QtPerPropertyTransition,
  type QtTransitionSpec,
} from "@qt-solid/core/native";
import type { QtMotionConfig } from "../../qt-intrinsics.ts";

import { createComponent as createQtComponent } from "../../runtime/renderer.ts";
import type {
  MotionComponentProps,
  MotionTarget,
  MotionTransition,
  MotionValue,
  TransitionSpec,
} from "./types.ts";
import { usePresence } from "./presence.ts";
import {
  OrchestrationContext,
  createOrchestration,
  useOrchestration,
  type OrchestrationParentControl,
} from "./orchestration.ts";

type MotionNodeHandle = QtNode & {
  setMotionTarget(
    target: QtMotionTarget,
    transition: QtPerPropertyTransition,
    delay?: number | null,
  ): void;
  setMotionConfig(config: QtMotionConfig): void;
  onMotionComplete(callback: () => void): void;
  setLayoutId(layoutId: string, transition?: TransitionSpec): void;
  unsetLayoutId(layoutId: string): void;
};

function isMotionNodeHandle(value: unknown): value is MotionNodeHandle {
  return (
    value != null &&
    typeof value === "object" &&
    "setMotionTarget" in value &&
    "setMotionConfig" in value
  );
}

// ---------------------------------------------------------------------------
// Gesture state — tracked by motion() HOC, consumed by bindMotionNode
// ---------------------------------------------------------------------------

interface GestureState {
  isHovered: Accessor<boolean>;
  isTapped: Accessor<boolean>;
  isFocused: Accessor<boolean>;
  isDragging: Accessor<boolean>;
}

/** Mutable drag controller — HOC allocates, bindMotionNode populates methods. */
interface DragController {
  onDown(x: number, y: number): void;
  onMove(x: number, y: number): void;
  onUp(x: number, y: number): void;
}

const MOTION_PROP_KEYS = [
  "initial", "animate", "exit", "transition",
  "whileHover", "whileTap", "whileFocus",
  "drag", "dragConstraints", "dragElastic",
  "onDragStart", "onDrag", "onDragEnd",
  "layout", "layoutId", "layoutTransition",
  "layer", "hitTest", "onAnimationComplete",
] as const

function splitMotionProps<Props extends object>(
  props: MotionComponentProps<Props>,
): { baseProps: Props; motionProps: MotionComponentProps<object> } {
  const [motionSlice, baseProps] = solidSplitProps(props, MOTION_PROP_KEYS);
  return {
    baseProps: baseProps as Props,
    motionProps: motionSlice as unknown as MotionComponentProps<object>,
  };
}

// -- Target lowering --

function lowerTarget(target: MotionTarget | undefined): QtMotionTarget {
  if (!target) {
    return {};
  }

  const resolve = (v: MotionValue | undefined): number | undefined =>
    Array.isArray(v) ? v[v.length - 1] : v;
  const resolveKf = (v: MotionValue | undefined): number[] | undefined =>
    Array.isArray(v) ? v : undefined;

  const bg = target.backgroundColor ? parseColor(target.backgroundColor) : undefined;
  const sc = target.shadowColor ? parseColor(target.shadowColor) : undefined;
  return {
    x: resolve(target.x),
    y: resolve(target.y),
    scaleX: resolve(target.scaleX ?? target.scale),
    scaleY: resolve(target.scaleY ?? target.scale),
    rotate: resolve(target.rotate),
    opacity: resolve(target.opacity),
    originX: resolve(target.originX),
    originY: resolve(target.originY),
    xKeyframes: resolveKf(target.x),
    yKeyframes: resolveKf(target.y),
    scaleXKeyframes: resolveKf(target.scaleX ?? target.scale),
    scaleYKeyframes: resolveKf(target.scaleY ?? target.scale),
    rotateKeyframes: resolveKf(target.rotate),
    opacityKeyframes: resolveKf(target.opacity),
    originXKeyframes: resolveKf(target.originX),
    originYKeyframes: resolveKf(target.originY),
    backgroundR: bg?.[0],
    backgroundG: bg?.[1],
    backgroundB: bg?.[2],
    backgroundA: bg?.[3],
    borderRadius: target.borderRadius,
    blurRadius: target.blur,
    shadowOffsetX: target.shadowOffsetX,
    shadowOffsetY: target.shadowOffsetY,
    shadowBlurRadius: target.shadowBlur,
    shadowR: sc?.[0],
    shadowG: sc?.[1],
    shadowB: sc?.[2],
    shadowA: sc?.[3],
  };
}

/** Parse CSS color string to [r, g, b, a] in 0.0-1.0 range. */
function parseColor(color: string): [number, number, number, number] | undefined {
  if (color.startsWith("#")) {
    const hex = color.slice(1);
    if (hex.length === 3) {
      const r = parseInt(hex[0]! + hex[0]!, 16) / 255;
      const g = parseInt(hex[1]! + hex[1]!, 16) / 255;
      const b = parseInt(hex[2]! + hex[2]!, 16) / 255;
      return [r, g, b, 1.0];
    }
    if (hex.length === 6 || hex.length === 8) {
      const r = parseInt(hex.slice(0, 2), 16) / 255;
      const g = parseInt(hex.slice(2, 4), 16) / 255;
      const b = parseInt(hex.slice(4, 6), 16) / 255;
      const a = hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1.0;
      return [r, g, b, a];
    }
  }
  return undefined;
}

// -- Transition lowering --

const NAMED_EASINGS: Record<string, [number, number, number, number]> = {
  linear: [0, 0, 1, 1],
  ease: [0.25, 0.1, 0.25, 1.0],
  "ease-in": [0.42, 0, 1, 1],
  "ease-out": [0, 0, 0.58, 1],
  "ease-in-out": [0.42, 0, 0.58, 1],
};

const SPRING_DEFAULT = { type: "spring" } as QtTransitionSpec;
const INSTANT_DEFAULT = { type: "instant" } as QtTransitionSpec;

function lowerTransitionSpec(spec: TransitionSpec | undefined): QtTransitionSpec | undefined {
  if (!spec) return undefined;

  const hasSpringFields = spec.stiffness != null
    || spec.damping != null
    || spec.mass != null
    || spec.velocity != null
    || spec.restDelta != null
    || spec.restSpeed != null;
  const hasTweenFields = spec.duration != null || spec.ease != null;
  const type = (spec.type === "spring"
    ? "spring"
    : spec.type === "instant"
      ? "instant"
      : hasSpringFields
        ? "spring"
        : hasTweenFields
          ? "tween"
          : "spring") as QtTransitionSpec["type"];

  if (type === "spring") {
    return {
      type,
      stiffness: spec.stiffness,
      damping: spec.damping,
      mass: spec.mass,
      velocity: spec.velocity,
      restDelta: spec.restDelta,
      restSpeed: spec.restSpeed,
    };
  }

  if (type === "instant") {
    return { type };
  }

  const ease = typeof spec.ease === "string"
    ? NAMED_EASINGS[spec.ease] ?? NAMED_EASINGS["ease-in-out"]
    : Array.isArray(spec.ease)
      ? (spec.ease as [number, number, number, number])
      : undefined;

  return {
    type,
    duration: spec.duration,
    ease: ease ? [...ease] : undefined,
    repeat: spec.repeat,
    repeatType: spec.repeatType,
    times: spec.times,
  };
}

function lowerTransition(transition: MotionTransition | undefined): QtPerPropertyTransition {
  if (!transition) {
    return {
      default: SPRING_DEFAULT,
    };
  }

  return {
    default: lowerTransitionSpec(transition.default ?? transition as TransitionSpec)
      ?? SPRING_DEFAULT,
    x: lowerTransitionSpec(transition.x),
    y: lowerTransitionSpec(transition.y),
    scaleX: lowerTransitionSpec(transition.scaleX),
    scaleY: lowerTransitionSpec(transition.scaleY),
    rotate: lowerTransitionSpec(transition.rotate),
    opacity: lowerTransitionSpec(transition.opacity),
    originX: lowerTransitionSpec(transition.originX),
    originY: lowerTransitionSpec(transition.originY),
  };
}

// -- Gesture target merge --

function mergeMotionTargets(
  base: MotionTarget | undefined,
  overlay: MotionTarget,
): MotionTarget {
  return {
    x: overlay.x ?? base?.x,
    y: overlay.y ?? base?.y,
    scale: overlay.scale ?? base?.scale,
    scaleX: overlay.scaleX ?? base?.scaleX,
    scaleY: overlay.scaleY ?? base?.scaleY,
    rotate: overlay.rotate ?? base?.rotate,
    opacity: overlay.opacity ?? base?.opacity,
    originX: overlay.originX ?? base?.originX,
    originY: overlay.originY ?? base?.originY,
    backgroundColor: overlay.backgroundColor ?? base?.backgroundColor,
    borderRadius: overlay.borderRadius ?? base?.borderRadius,
    blur: overlay.blur ?? base?.blur,
    shadowOffsetX: overlay.shadowOffsetX ?? base?.shadowOffsetX,
    shadowOffsetY: overlay.shadowOffsetY ?? base?.shadowOffsetY,
    shadowBlur: overlay.shadowBlur ?? base?.shadowBlur,
    shadowColor: overlay.shadowColor ?? base?.shadowColor,
  };
}

// -- Sending target to Rust --
// No JS-side deduplication. Rust's NodeTimeline already skips channels
// whose target hasn't changed, so redundant sends are cheap no-ops.

function sendTarget(
  node: MotionNodeHandle,
  target: MotionTarget | undefined,
  transition: MotionTransition | undefined,
  delay?: number,
  label?: string,
) {
  const lowered = lowerTarget(target);
  if (process.env.QT_SOLID_MOTION_TRACE) {
    console.log(`[qt-motion:js] ${label ?? 'send'} target=`, lowered, `delay=${delay ?? 0}`);
  }
  node.setMotionTarget(
    lowered,
    lowerTransition(transition),
    (delay ?? 0) > 0 ? delay : undefined,
  );
}

// ---------------------------------------------------------------------------
// Gesture event wrapping — HOC prop composition
// ---------------------------------------------------------------------------

type EventHandler = ((...args: unknown[]) => void) | undefined;
type GestureHandler = ((...args: unknown[]) => void) | undefined;

/**
 * Chain two event handlers: call `before` first (forwarding args), then `original`.
 */
function chainHandler(
  before: GestureHandler,
  original: EventHandler,
): EventHandler {
  if (!before) return original;
  if (!original) return before as EventHandler;
  return (...args: unknown[]) => {
    (before as (...a: unknown[]) => void)(...args);
    (original as (...a: unknown[]) => void)(...args);
  };
}

/**
 * Wrap base props with gesture event handlers for motion tracking.
 * Chains motion's gesture handlers before the component's own handlers.
 */
function injectGestureHandlers<Props extends object>(
  baseProps: Props,
  gestureHandlers: {
    onPointerEnter?: GestureHandler;
    onPointerLeave?: GestureHandler;
    onPointerDown?: GestureHandler;
    onPointerMove?: GestureHandler;
    onPointerUp?: GestureHandler;
    onFocusIn?: GestureHandler;
    onFocusOut?: GestureHandler;
  },
): Props {
  const src = baseProps as Record<string, unknown>;
  const descriptors = Object.getOwnPropertyDescriptors(baseProps) as Record<string, PropertyDescriptor>;

  const eventPairs: [string, GestureHandler][] = [
    ["onPointerEnter", gestureHandlers.onPointerEnter],
    ["onPointerLeave", gestureHandlers.onPointerLeave],
    ["onPointerDown", gestureHandlers.onPointerDown],
    ["onPointerMove", gestureHandlers.onPointerMove],
    ["onPointerUp", gestureHandlers.onPointerUp],
    ["onFocusIn", gestureHandlers.onFocusIn],
    ["onFocusOut", gestureHandlers.onFocusOut],
  ];

  for (const [name, gestureHandler] of eventPairs) {
    if (!gestureHandler) continue;
    const existingDescriptor = descriptors[name];

    if (existingDescriptor && "get" in existingDescriptor && existingDescriptor.get) {
      const originalGet = existingDescriptor.get;
      descriptors[name] = {
        configurable: true,
        enumerable: true,
        get() {
          return chainHandler(gestureHandler, originalGet() as EventHandler);
        },
      };
    } else {
      descriptors[name] = {
        configurable: true,
        enumerable: true,
        get() {
          return chainHandler(gestureHandler, src[name] as EventHandler);
        },
      };
    }
  }

  return Object.defineProperties({}, descriptors) as Props;
}

// -- Drag helpers --

function resolveMotionValue(v: MotionValue | undefined): number | undefined {
  if (v == null) return undefined;
  return Array.isArray(v) ? v[v.length - 1] : v;
}

function clamp(value: number, min: number | undefined, max: number | undefined): number {
  if (min != null && value < min) return min;
  if (max != null && value > max) return max;
  return value;
}

function applyElastic(
  value: number,
  min: number | undefined,
  max: number | undefined,
  elastic: number,
): number {
  if (min != null && value < min) {
    return min + (value - min) * elastic;
  }
  if (max != null && value > max) {
    return max + (value - max) * elastic;
  }
  return value;
}

// -- Binding --

function bindMotionNode(
  node: MotionNodeHandle,
  readMotion: () => MotionComponentProps<object>,
  gesture: GestureState,
  dragCtrl: DragController,
): void {
  let started = false;
  const presence = usePresence();
  const parentOrch = useOrchestration();
  const childIndex = parentOrch?.registerChild() ?? 0;

  let userOnComplete: (() => void) | undefined;
  createEffect(() => {
    userOnComplete = readMotion().onAnimationComplete;
  });

  const writeConfig = () => {
    const props = readMotion();
    const layoutEnabled = props.layout === true || props.layout === "position" || props.layout === "size";
    const enabled =
      props.layer === true ||
      layoutEnabled ||
      props.initial != null ||
      props.animate != null;
    const hasGestures = props.whileHover != null || props.whileTap != null
      || props.whileFocus != null || props.drag != null;
    node.setMotionConfig({
      layerEnabled: enabled,
      layoutEnabled,
      hitTestEnabled: props.hitTest === true || hasGestures,
    });
  };

  writeConfig();

  // Set initial pose immediately (snap, no animation)
  const initialTarget = lowerTarget(readMotion().initial ?? readMotion().animate);
  node.setMotionTarget(initialTarget, { default: INSTANT_DEFAULT });

  createEffect(() => {
    writeConfig();
  });

  // ---------------------------------------------------------------------------
  // Drag gesture binding
  // ---------------------------------------------------------------------------
  // Persistent drag pose — survives across drag sessions, prevents animate
  // effect from overwriting drag-controlled axes on release.
  let dragPoseX = resolveMotionValue(readMotion().initial?.x ?? readMotion().animate?.x) ?? 0;
  let dragPoseY = resolveMotionValue(readMotion().initial?.y ?? readMotion().animate?.y) ?? 0;
  let hasDragPose = false;

  {
    let dragStartPointerX = 0;
    let dragStartPointerY = 0;
    let dragOriginX = 0;
    let dragOriginY = 0;

    const [isDragging, setIsDragging] = createSignal(false);
    (gesture as { isDragging: Accessor<boolean> }).isDragging = isDragging;

    dragCtrl.onDown = (px: number, py: number) => {
      const props = readMotion();
      if (!props.drag) return;

      dragStartPointerX = px;
      dragStartPointerY = py;
      dragOriginX = dragPoseX;
      dragOriginY = dragPoseY;

      setIsDragging(true);
      props.onDragStart?.(px, py);
    };

    dragCtrl.onMove = (px: number, py: number) => {
      if (!isDragging()) return;
      const props = readMotion();
      const axis = props.drag;
      const elastic = props.dragElastic ?? 0.35;
      const constraints = props.dragConstraints;

      let dx = px - dragStartPointerX;
      let dy = py - dragStartPointerY;
      if (axis === "x") dy = 0;
      if (axis === "y") dx = 0;

      let targetX = dragOriginX + dx;
      let targetY = dragOriginY + dy;

      if (constraints) {
        targetX = applyElastic(targetX, constraints.left, constraints.right, elastic);
        targetY = applyElastic(targetY, constraints.top, constraints.bottom, elastic);
      }

      dragPoseX = targetX;
      dragPoseY = targetY;
      hasDragPose = true;

      node.setMotionTarget(
        { x: targetX, y: targetY },
        { default: INSTANT_DEFAULT },
      );
      props.onDrag?.(px, py);
    };

    dragCtrl.onUp = (px: number, py: number) => {
      if (!isDragging()) return;
      const props = readMotion();

      const axis = props.drag;
      const constraints = props.dragConstraints;
      let dx = px - dragStartPointerX;
      let dy = py - dragStartPointerY;
      if (axis === "x") dy = 0;
      if (axis === "y") dx = 0;

      let finalX = dragOriginX + dx;
      let finalY = dragOriginY + dy;

      if (constraints) {
        finalX = clamp(finalX, constraints.left, constraints.right);
        finalY = clamp(finalY, constraints.top, constraints.bottom);
      }

      dragPoseX = finalX;
      dragPoseY = finalY;
      hasDragPose = true;

      // Release with spring — velocity inferred automatically by motion crate
      const releaseTransition = lowerTransition(props.transition) ?? { default: SPRING_DEFAULT };
      node.setMotionTarget({ x: finalX, y: finalY }, releaseTransition);

      // Signal after commanding release so animate effect doesn't race
      setIsDragging(false);
      props.onDragEnd?.(px, py);
    };
  }

  // layoutId: register for shared layout animation
  createEffect(() => {
    const props = readMotion();
    const layoutId = props.layoutId;
    if (!layoutId) return;
    node.setLayoutId(layoutId, props.layoutTransition);
    onCleanup(() => node.unsetLayoutId(layoutId));
  });

  // layout (same-element FLIP): detect own bounds changes and animate via FLIP
  {
    const layoutMode = () => {
      const l = readMotion().layout;
      if (l === true || l === "position" || l === "size") return l;
      return null;
    };

    createEffect(() => {
      const mode = layoutMode();
      if (!mode) return;

      const fragNode = node as unknown as { canvasNodeId: number; fragmentId: number; eventHandlers: Map<string, (...args: unknown[]) => void> };
      const LISTENER_LAYOUT = 1;

      let prevX = 0;
      let prevY = 0;
      let prevW = 0;
      let prevH = 0;
      let hasPrev = false;

      // Snapshot initial bounds
      const initialBounds = canvasFragmentGetWorldBounds(fragNode.canvasNodeId, fragNode.fragmentId);
      if (initialBounds) {
        prevX = initialBounds.x;
        prevY = initialBounds.y;
        prevW = initialBounds.width;
        prevH = initialBounds.height;
        hasPrev = true;
      }

      const onLayoutHandler = (_e: { x: number; y: number; width: number; height: number }) => {
        // Get new world bounds (layout event gives local position/size, need world)
        const newBounds = canvasFragmentGetWorldBounds(fragNode.canvasNodeId, fragNode.fragmentId);
        if (!newBounds || !hasPrev) {
          if (newBounds) {
            prevX = newBounds.x;
            prevY = newBounds.y;
            prevW = newBounds.width;
            prevH = newBounds.height;
            hasPrev = true;
          }
          return;
        }

        const curMode = untrack(layoutMode);
        const dx = prevX - newBounds.x;
        const dy = prevY - newBounds.y;
        const sx = newBounds.width > 0 ? prevW / newBounds.width : 1;
        const sy = newBounds.height > 0 ? prevH / newBounds.height : 1;

        // Filter by mode
        const posDelta = curMode !== "size" ? (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) : false;
        const sizeDelta = curMode !== "position" ? (Math.abs(sx - 1) > 0.001 || Math.abs(sy - 1) > 0.001) : false;

        if (posDelta || sizeDelta) {
          const flipDx = curMode === "size" ? 0 : dx;
          const flipDy = curMode === "size" ? 0 : dy;
          const flipSx = curMode === "position" ? 1 : sx;
          const flipSy = curMode === "position" ? 1 : sy;

          const transition = lowerTransitionSpec(readMotion().layoutTransition) ?? {
            type: "spring",
            stiffness: 500,
            damping: 30,
            mass: 1,
          } as QtTransitionSpec;

          canvasFragmentSetLayoutFlip(
            fragNode.canvasNodeId,
            fragNode.fragmentId,
            flipDx, flipDy, flipSx, flipSy,
            transition,
          );
        }

        prevX = newBounds.x;
        prevY = newBounds.y;
        prevW = newBounds.width;
        prevH = newBounds.height;
      };

      // Register layout listener
      fragNode.eventHandlers.set("onLayout", onLayoutHandler as (...args: unknown[]) => void);
      canvasFragmentSetListener(fragNode.canvasNodeId, fragNode.fragmentId, LISTENER_LAYOUT, true);

      onCleanup(() => {
        fragNode.eventHandlers.delete("onLayout");
        canvasFragmentSetListener(fragNode.canvasNodeId, fragNode.fragmentId, LISTENER_LAYOUT, false);
      });
    });
  }

  onMount(() => {
    started = true;
    // Compute delay dynamically from current orchestration config
    const delay = parentOrch?.getChildDelay(childIndex) ?? 0;

    if (parentOrch && !untrack(parentOrch.childrenCanAnimate)) {
      // when: "beforeChildren" — wait for parent to unlock us
      createEffect(
        on(parentOrch.childrenCanAnimate, (canAnimate) => {
          if (!canAnimate) return;
          sendTarget(node, readMotion().animate, readMotion().transition, delay, 'orch-unlock');
          node.onMotionComplete(() => {
            userOnComplete?.();
            parentOrch.onChildComplete();
          });
        }),
      );
    } else {
      sendTarget(node, readMotion().animate, readMotion().transition, delay, 'mount');
      if (parentOrch) {
        node.onMotionComplete(() => {
          userOnComplete?.();
          parentOrch.onChildComplete();
        });
      } else {
        node.onMotionComplete(() => {
          userOnComplete?.();
        });
      }
    }
  });

  // Track animate prop changes + gesture overlays
  // Priority: drag > tap > hover > focus > animate
  createEffect(() => {
    const props = readMotion();
    const mounted = presence?.mount() ?? true;
    if (!started || !mounted) return;
    // While dragging, drag controller drives x/y directly — suppress animate
    if (gesture.isDragging()) return;

    const base = props.animate;

    // Select overlay by gesture priority: tap > hover > focus
    let overlay: MotionTarget | undefined;
    if (gesture.isTapped() && props.whileTap) {
      overlay = props.whileTap;
    } else if (gesture.isHovered() && props.whileHover) {
      overlay = props.whileHover;
    } else if (gesture.isFocused() && props.whileFocus) {
      overlay = props.whileFocus;
    }

    let effective = overlay ? mergeMotionTargets(base, overlay) : base;

    // Preserve drag-controlled axes so animate doesn't overwrite release spring
    if (hasDragPose && props.drag) {
      const axis = props.drag;
      effective = { ...effective };
      if (axis === true || axis === "x") effective.x = dragPoseX;
      if (axis === true || axis === "y") effective.y = dragPoseY;
    }

    const delay = parentOrch?.getChildDelay(childIndex) ?? 0;
    sendTarget(node, effective, props.transition, delay, 'animate-effect');
  });

  // Exit animation: triggered by PresenceContext
  if (presence) {
    createEffect(
      on(() => presence.mount(), (mounted) => {
        if (mounted || !started) return;
        const exitTarget = readMotion().exit;
        if (!exitTarget) {
          // No exit animation — signal complete immediately
          presence.onExitComplete();
          return;
        }
        sendTarget(node, exitTarget, readMotion().transition, undefined, 'exit');
        node.onMotionComplete(() => {
          userOnComplete?.();
          if (!presence.mount()) {
            presence.onExitComplete();
          }
        });
      }),
    );
  }
}

export const __testMotionInternals = {
  lowerTarget,
  lowerTransition,
  lowerTransitionSpec,
  mergeMotionTargets,
};

export function motion<Props extends object>(
  component: Component<Props>,
): Component<MotionComponentProps<Props>> {
  const MotionComponent: Component<MotionComponentProps<Props>> = (props) => {
    const split = createMemo(() => splitMotionProps(props));

    // Gesture state signals — driven by chained event handlers
    const [isHovered, setIsHovered] = createSignal(false);
    const [isTapped, setIsTapped] = createSignal(false);
    const [isFocused, setIsFocused] = createSignal(false);
    // isDragging is populated by bindMotionNode's drag binding
    const [isDragging] = createSignal(false);

    const gestureState: GestureState = { isHovered, isTapped, isFocused, isDragging };

    // Drag controller — methods populated by bindMotionNode
    const dragCtrl: DragController = {
      onDown: () => {},
      onMove: () => {},
      onUp: () => {},
    };

    // Determine if any gesture/drag props are present
    const hasGestureProps = createMemo(() => {
      const m = split().motionProps;
      return m.whileHover != null || m.whileTap != null || m.whileFocus != null || m.drag != null;
    });

    // Wrap base props with gesture event handlers when gesture props exist
    const enhancedBaseProps = createMemo(() => {
      const base = split().baseProps;
      if (!hasGestureProps()) return base;

      return injectGestureHandlers(base, {
        onPointerEnter: () => setIsHovered(true),
        onPointerLeave: () => { setIsHovered(false); setIsTapped(false); },
        onPointerDown: (ev: unknown) => {
          setIsTapped(true);
          const { x, y } = ev as { x: number; y: number };
          dragCtrl.onDown(x, y);
        },
        onPointerMove: (ev: unknown) => {
          const { x, y } = ev as { x: number; y: number };
          dragCtrl.onMove(x, y);
        },
        onPointerUp: (ev: unknown) => {
          setIsTapped(false);
          const { x, y } = ev as { x: number; y: number };
          dragCtrl.onUp(x, y);
        },
        onFocusIn: () => setIsFocused(true),
        onFocusOut: () => setIsFocused(false),
      });
    });

    const element = createQtComponent(component, enhancedBaseProps());

    if (!isMotionNodeHandle(element)) {
      throw new Error(
        "motion(Component) requires a component with a single native node root",
      );
    }

    bindMotionNode(
      element,
      () => split().motionProps,
      gestureState,
      dragCtrl,
    );

    // Provide orchestration context to children if transition has stagger/when
    const transition = createMemo(() => split().motionProps.transition);
    const needsOrchestration = createMemo(() => {
      const t = transition();
      return (t?.staggerChildren ?? 0) > 0
        || (t?.delayChildren ?? 0) > 0
        || (t?.when != null && t.when !== false);
    });

    if (untrack(needsOrchestration)) {
      const t = untrack(transition)!;
      const orch = createOrchestration({
        delayChildren: t.delayChildren ?? 0,
        staggerChildren: t.staggerChildren ?? 0,
        when: t.when ?? false,
      }) as OrchestrationParentControl;

      if (t.when === "beforeChildren") {
        element.onMotionComplete(() => orch.unlockChildren());
      }

      if (t.when === "afterChildren") {
        createEffect(
          on(orch.allChildrenComplete, (done) => {
            if (!done) return;
            const motionProps = split().motionProps;
            sendTarget(element, motionProps.animate, motionProps.transition, undefined, 'afterChildren');
          }),
        );
      }

      return OrchestrationContext.Provider({
        value: orch,
        get children() { return element; },
      }) as unknown as JSX.Element;
    }

    return element;
  };

  return MotionComponent;
}
