export type MotionValue = number | number[];

export interface MotionTarget {
  x?: MotionValue;
  y?: MotionValue;
  scale?: MotionValue;
  scaleX?: MotionValue;
  scaleY?: MotionValue;
  rotate?: MotionValue;
  opacity?: MotionValue;
  originX?: MotionValue;
  originY?: MotionValue;
  backgroundColor?: string;
  borderRadius?: number;
  blur?: number;
  shadowOffsetX?: number;
  shadowOffsetY?: number;
  shadowBlur?: number;
  shadowColor?: string;
}

export type NamedEasing = "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out";
export type BezierEasing = [number, number, number, number];

export interface TransitionSpec {
  type?: "tween" | "spring" | "instant";
  duration?: number;
  ease?: NamedEasing | BezierEasing;
  stiffness?: number;
  damping?: number;
  mass?: number;
  velocity?: number;
  restDelta?: number;
  restSpeed?: number;
  repeat?: number;
  repeatType?: "loop" | "reverse";
  times?: number[];
}

export interface MotionTransition extends TransitionSpec {
  default?: TransitionSpec;
  x?: TransitionSpec;
  y?: TransitionSpec;
  scaleX?: TransitionSpec;
  scaleY?: TransitionSpec;
  rotate?: TransitionSpec;
  opacity?: TransitionSpec;
  originX?: TransitionSpec;
  originY?: TransitionSpec;
  staggerChildren?: number;
  delayChildren?: number;
  when?: "beforeChildren" | "afterChildren" | false;
}

export interface DragConstraints {
  left?: number;
  right?: number;
  top?: number;
  bottom?: number;
}

export interface MotionProps {
  initial?: MotionTarget;
  animate?: MotionTarget;
  exit?: MotionTarget;
  transition?: MotionTransition;
  whileHover?: MotionTarget;
  whileTap?: MotionTarget;
  whileFocus?: MotionTarget;
  drag?: boolean | "x" | "y";
  dragConstraints?: DragConstraints;
  dragElastic?: number;
  onDragStart?: (x: number, y: number) => void;
  onDrag?: (x: number, y: number) => void;
  onDragEnd?: (x: number, y: number) => void;
  layout?: boolean | "position" | "size";
  layoutId?: string;
  layoutTransition?: TransitionSpec;
  layer?: boolean;
  hitTest?: boolean;
  onAnimationComplete?: () => void;
}

export type MotionComponentProps<Props extends object> = Props & MotionProps;
