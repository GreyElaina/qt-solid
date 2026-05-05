// Internal — consumed by renderer, not public API
export {
  bindMotionNode,
  isMotionNodeHandle,
  MOTION_PROP_KEYS,
} from "./motion.ts";
export type {
  MotionNodeHandle,
  GestureState,
  DragController,
} from "./motion.ts";

// Test-only
export { __testMotionInternals } from "./motion.ts";

// Public API
export { useMotionValue } from "./use-motion-value.ts";
export type { MotionValueConfig } from "./use-motion-value.ts";
export { createVariants } from "./variants.ts";
export { AnimatePresence } from "./presence.ts";
export type {
  MotionTarget,
  MotionTransition,
  MotionValue,
  MotionProps,
  NamedEasing,
  BezierEasing,
  TransitionSpec,
  DragConstraints,
} from "./types.ts";
