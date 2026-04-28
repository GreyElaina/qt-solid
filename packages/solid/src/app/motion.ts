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
import type {
  QtMotionTarget,
  QtNode,
  QtPerPropertyTransition,
  QtTransitionSpec,
} from "@qt-solid/core/native";
import type { QtMotionConfig } from "../qt-intrinsics.ts";

import { createComponent as createQtComponent } from "../runtime/renderer.ts";
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

/**
 * Chain two event handlers: call `before` first, then `original`.
 * If either is undefined, return the other.
 */
function chainHandler(
  before: (() => void) | undefined,
  original: EventHandler,
): EventHandler {
  if (!before) return original;
  if (!original) return before as EventHandler;
  return (...args: unknown[]) => {
    before();
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
    onPointerEnter: () => void;
    onPointerLeave: () => void;
    onPointerDown: () => void;
    onPointerUp: () => void;
    onFocusIn: () => void;
    onFocusOut: () => void;
  },
): Props {
  const src = baseProps as Record<string, unknown>;
  const descriptors = Object.getOwnPropertyDescriptors(baseProps) as Record<string, PropertyDescriptor>;

  const eventPairs: [string, () => void][] = [
    ["onPointerEnter", gestureHandlers.onPointerEnter],
    ["onPointerLeave", gestureHandlers.onPointerLeave],
    ["onPointerDown", gestureHandlers.onPointerDown],
    ["onPointerUp", gestureHandlers.onPointerUp],
    ["onFocusIn", gestureHandlers.onFocusIn],
    ["onFocusOut", gestureHandlers.onFocusOut],
  ];

  for (const [name, gestureHandler] of eventPairs) {
    const existingDescriptor = descriptors[name];

    if (existingDescriptor && "get" in existingDescriptor && existingDescriptor.get) {
      // Reactive getter — wrap with lazy chaining
      const originalGet = existingDescriptor.get;
      descriptors[name] = {
        configurable: true,
        enumerable: true,
        get() {
          return chainHandler(gestureHandler, originalGet() as EventHandler);
        },
      };
    } else {
      // Static value or absent — chain with current value
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

// -- Binding --

function bindMotionNode(
  node: MotionNodeHandle,
  readMotion: () => MotionComponentProps<object>,
  gesture: GestureState,
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

  // layoutId: register for shared layout animation
  createEffect(() => {
    const props = readMotion();
    const layoutId = props.layoutId;
    if (!layoutId) return;
    node.setLayoutId(layoutId, props.layoutTransition);
    onCleanup(() => node.unsetLayoutId(layoutId));
  });

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
  // Priority: tap > hover > focus > animate
  // Gesture state is driven by the HOC's event handler wiring,
  // not by the presence of props.
  createEffect(() => {
    const props = readMotion();
    const mounted = presence?.mount() ?? true;
    if (!started || !mounted) return;

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

    const effective = overlay ? mergeMotionTargets(base, overlay) : base;
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

    const gestureState: GestureState = { isHovered, isTapped, isFocused };

    // Determine if any gesture props are present (static check)
    const hasGestureProps = createMemo(() => {
      const m = split().motionProps;
      return m.whileHover != null || m.whileTap != null || m.whileFocus != null;
    });

    // Wrap base props with gesture event handlers when gesture props exist
    const enhancedBaseProps = createMemo(() => {
      const base = split().baseProps;
      if (!hasGestureProps()) return base;

      return injectGestureHandlers(base, {
        onPointerEnter: () => setIsHovered(true),
        onPointerLeave: () => { setIsHovered(false); setIsTapped(false); },
        onPointerDown: () => setIsTapped(true),
        onPointerUp: () => setIsTapped(false),
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
      }) as unknown as JSX.Element; // Provider returns JSX.Element but wrapped type
    }

    return element;
  };

  return MotionComponent;
}
