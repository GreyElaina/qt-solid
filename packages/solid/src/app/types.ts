import type { QtApp, QtHostEvent } from "@qt-solid/core";
import type { QtWidgetCapture } from "@qt-solid/core/native";
import type {
  ViewIntrinsicProps,
  WindowIntrinsicProps,
} from "../qt-intrinsics.ts";
import type { Accessor, JSX } from "solid-js";

export type WidgetGeometry = Pick<
  ViewIntrinsicProps,
  "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "maxHeight" | "aspectRatio"
>;
export type WidgetFlex = Pick<ViewIntrinsicProps, "grow" | "shrink" | "basis" | "alignSelf" | "margin">;
export type WidgetState = Pick<ViewIntrinsicProps, "enabled" | "hidden">;
export type ViewLayout = Pick<
  ViewIntrinsicProps,
  "direction" | "justifyContent" | "alignItems" | "gap" | "padding" | "wrap"
>;

export interface WidgetProps {
  ref?: (node: { readonly id: number }) => void;
  width?: WidgetGeometry["width"];
  height?: WidgetGeometry["height"];
  minWidth?: WidgetGeometry["minWidth"];
  minHeight?: WidgetGeometry["minHeight"];
  flexGrow?: WidgetFlex["grow"];
  flexShrink?: WidgetFlex["shrink"];
  enabled?: WidgetState["enabled"];
  onHoverEnter?: () => void;
  onHoverLeave?: () => void;
}

export interface FlexChildProps {
  maxWidth?: WidgetGeometry["maxWidth"];
  maxHeight?: WidgetGeometry["maxHeight"];
  aspectRatio?: WidgetGeometry["aspectRatio"];
  flexBasis?: WidgetFlex["basis"];
  alignSelf?: WidgetFlex["alignSelf"];
  margin?: WidgetFlex["margin"];
  hidden?: WidgetState["hidden"];
}

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

export interface ViewProps extends WidgetProps, FlexChildProps {
  direction?: ViewLayout["direction"];
  justifyContent?: ViewLayout["justifyContent"];
  alignItems?: ViewLayout["alignItems"];
  gap?: ViewLayout["gap"];
  padding?: ViewLayout["padding"];
  wrap?: ViewLayout["wrap"];
  backgroundColor?: ViewIntrinsicProps["backgroundColor"];
  children?: JSX.Element;
}

export interface WindowProps extends ViewProps {
  title?: WindowIntrinsicProps["title"];
  visible?: WindowIntrinsicProps["visible"];
  frameless?: WindowIntrinsicProps["frameless"];
  transparentBackground?: WindowIntrinsicProps["transparentBackground"];
  alwaysOnTop?: WindowIntrinsicProps["alwaysOnTop"];
  onCloseRequested?: WindowIntrinsicProps["onCloseRequested"];
}

export interface PopupDismissEvent {
  stopPropagation(): void
}

export interface PopupProps extends ViewProps {
  visible?: boolean;
  anchor?: { readonly id: number };
  placement?: "bottom" | "top" | "right" | "left";
  screenX?: number;
  screenY?: number;
  onDismiss?: (event: PopupDismissEvent) => void;
}

export type PopupSource = PopupProps | Accessor<PopupProps>;

export interface TooltipProps {
  anchor?: { readonly id: number };
  placement?: "bottom" | "top" | "right" | "left";
  hoverDelay?: number;
  hideDelay?: number;
  children?: JSX.Element;
}

export type WindowSource = WindowProps | Accessor<WindowProps>;
export type WindowComposable = (children: Accessor<JSX.Element>) => JSX.Element;
export type WindowConfig = WindowProps;
export type WindowConfigSource = WindowSource;

export interface RenderQtOptions {
  attachNativeEvents?: (handleEvent: (event: QtHostEvent) => void) => void;
}

export interface AppMountOptions extends RenderQtOptions {}

export interface AppMount {
  dispose(): void;
}

export interface WindowAllClosedContext {
  quit(): void;
}

export interface CreateAppOptions {
  render(): JSX.Element;
  onActivate?: () => void;
  onWindowAllClosed?: (context: WindowAllClosedContext) => void;
}

export type AppDefinition = WindowHandle | CreateAppOptions;
export type AppFactory = () => AppDefinition;

export interface AppHandle {
  mount(app: QtApp, options?: AppMountOptions): AppMount;
}

export interface WindowHandle {
  render(): JSX.Element;
  renderQt(app: QtApp, options?: RenderQtOptions): () => void;
  dispose(): void;
  open(): void;
  requestRepaint(): void;
  requestNextFrame(): void;
  frameState(): WindowFrameState;
  capture(): QtWidgetCapture;
}

export interface WindowFrameState {
  seq: number;
  elapsedMs: number;
  deltaMs: number;
}
