import type { QtApp, QtHostEvent } from "@qt-solid/core";
import type { QtWidgetCapture } from "@qt-solid/core/native";
import type { Accessor, JSX } from "solid-js";

// ---------------------------------------------------------------------------
// Widget-level props — used by native widget nodes (Window, Canvas, Popup)
// ---------------------------------------------------------------------------

export interface WidgetProps {
  ref?: (node: { readonly id: number }) => void;
  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
  maxWidth?: number;
  maxHeight?: number;
  flexGrow?: number;
  flexShrink?: number;
  flexBasis?: number;
  alignSelf?: string;
  margin?: number;
  enabled?: boolean;
  hidden?: boolean;
  onHoverEnter?: () => void;
  onHoverLeave?: () => void;
}

export interface ViewProps extends WidgetProps {
  direction?: "column" | "row";
  justifyContent?: string;
  alignItems?: string;
  gap?: number;
  padding?: number;
  wrap?: "nowrap" | "wrap" | "wrap-reverse";
  backgroundColor?: string;
  children?: JSX.Element;
}

// ---------------------------------------------------------------------------
// App & window types
// ---------------------------------------------------------------------------

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
