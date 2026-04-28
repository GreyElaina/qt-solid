import {
  createMemo,
  splitProps as solidSplitProps,
  mergeProps as solidMergeProps,
  type Component,
  type JSX,
} from "solid-js";
import { createComponent as createQtComponent } from "../runtime/renderer.ts";
import type {
  MotionTarget,
  MotionTransition,
  MotionComponentProps,
  MotionProps,
} from "./types.ts";

// -- Type machinery --

/** A single variant axis: name → target mapping. */
type VariantAxis = Record<string, MotionTarget>;

/** The full variants definition: axis name → axis. */
type VariantsDefinition = Record<string, VariantAxis>;

/**
 * For each axis in V, produce a prop whose value is one of its keys.
 * e.g. `{ visibility: "hidden" | "shown" }`
 */
type VariantProps<V extends VariantsDefinition> = {
  [K in keyof V]?: keyof V[K] & string;
};

type CompoundVariantEntry<V extends VariantsDefinition> = VariantProps<V> & {
  target: MotionTarget;
};

interface CreateVariantsConfig<V extends VariantsDefinition> {
  /** Applied to every variant combination as the base layer. */
  base?: MotionTarget;
  /** Variant axes. */
  variants: V;
  /** Which variant value each axis starts at (also used as `initial`). */
  defaultVariants?: VariantProps<V>;
  /** Override target when specific variant combinations match. */
  compoundVariants?: CompoundVariantEntry<V>[];
  /** Default transition for all variant-driven animations. */
  transition?: MotionTransition;
}

type VariantPassthroughProps = Pick<MotionProps, "layer" | "hitTest" | "layout" | "layoutId">;

/**
 * CVA-style motion variant factory.
 *
 * Wraps a `motion(Component)` and returns a new component that accepts
 * variant axis names as props. The resolved variant target is fed into
 * the motion system as `animate`; `initial` comes from `defaultVariants`.
 *
 * ```ts
 * const Card = createVariants(motion(View), {
 *   base: { opacity: 1 },
 *   variants: {
 *     state: { idle: { y: 0 }, lifted: { y: -8, scale: 1.02 } },
 *   },
 *   defaultVariants: { state: "idle" },
 *   transition: { type: "spring", stiffness: 300, damping: 20 },
 * });
 *
 * <Card state="lifted" width={200} />
 * ```
 */
export function createVariants<
  Props extends object,
  V extends VariantsDefinition,
>(
  component: Component<MotionComponentProps<Props>>,
  config: CreateVariantsConfig<V>,
): Component<Props & VariantProps<V> & VariantPassthroughProps> {
  const axisKeys = Object.keys(config.variants) as (keyof V & string)[];
  const initialTarget = resolveTarget(config, config.defaultVariants ?? {} as VariantProps<V>);

  const VariantComponent = (props: Props & VariantProps<V> & VariantPassthroughProps): JSX.Element => {
    const [variantSlice, baseSlice] = solidSplitProps(props, axisKeys);

    const animateTarget = createMemo(() =>
      resolveTarget(config, variantSlice as VariantProps<V>),
    );

    // mergeProps preserves reactivity — baseSlice is a Solid Proxy,
    // and the override object uses getters for motion props.
    const componentProps = solidMergeProps(baseSlice, {
      get initial() { return initialTarget; },
      get animate() { return animateTarget(); },
      get transition() { return config.transition; },
    }) as MotionComponentProps<Props>;

    return createQtComponent(component, componentProps);
  };

  return VariantComponent;
}

// -- Resolution --

function resolveTarget<V extends VariantsDefinition>(
  config: CreateVariantsConfig<V>,
  selection: VariantProps<V>,
): MotionTarget {
  let target: MotionTarget = { ...config.base };

  const axes = config.variants;
  for (const axis of Object.keys(axes) as (keyof V & string)[]) {
    const value = selection[axis] ?? config.defaultVariants?.[axis];
    if (value == null) continue;
    const axisTargets = axes[axis];
    if (!axisTargets) continue;
    const resolved = axisTargets[value as string];
    if (resolved) {
      target = mergeTargets(target, resolved);
    }
  }

  if (config.compoundVariants) {
    for (const compound of config.compoundVariants) {
      if (compoundMatches(compound, selection, config.defaultVariants)) {
        target = mergeTargets(target, compound.target);
      }
    }
  }

  return target;
}

function compoundMatches<V extends VariantsDefinition>(
  compound: CompoundVariantEntry<V>,
  selection: VariantProps<V>,
  defaults?: VariantProps<V>,
): boolean {
  for (const key of Object.keys(compound) as (keyof V & string)[]) {
    if (key === "target") continue;
    const expected = compound[key];
    const actual = selection[key] ?? defaults?.[key];
    if (actual !== expected) return false;
  }
  return true;
}

function mergeTargets(base: MotionTarget, overlay: MotionTarget): MotionTarget {
  return {
    x: overlay.x ?? base.x,
    y: overlay.y ?? base.y,
    scale: overlay.scale ?? base.scale,
    scaleX: overlay.scaleX ?? base.scaleX,
    scaleY: overlay.scaleY ?? base.scaleY,
    rotate: overlay.rotate ?? base.rotate,
    opacity: overlay.opacity ?? base.opacity,
    originX: overlay.originX ?? base.originX,
    originY: overlay.originY ?? base.originY,
    backgroundColor: overlay.backgroundColor ?? base.backgroundColor,
    borderRadius: overlay.borderRadius ?? base.borderRadius,
    blur: overlay.blur ?? base.blur,
    shadowOffsetX: overlay.shadowOffsetX ?? base.shadowOffsetX,
    shadowOffsetY: overlay.shadowOffsetY ?? base.shadowOffsetY,
    shadowBlur: overlay.shadowBlur ?? base.shadowBlur,
    shadowColor: overlay.shadowColor ?? base.shadowColor,
  };
}
