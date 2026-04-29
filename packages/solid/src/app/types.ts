import type { QtApp, QtHostEvent } from "@qt-solid/core";
import type { QtWidgetCapture } from "@qt-solid/core/native";
import type {
  ViewIntrinsicProps,
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
