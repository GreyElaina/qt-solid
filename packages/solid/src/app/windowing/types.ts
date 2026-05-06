import type { Accessor, JSX } from "solid-js";
import type { QtNode } from "@qt-solid/core/native";
import type { ViewProps } from "../types.ts";

export interface WindowProps extends ViewProps {
  title?: string;
  visible?: boolean;
  frameless?: boolean;
  transparentBackground?: boolean;
  alwaysOnTop?: boolean;
  gpu?: boolean;
  onCloseRequested?: () => void;
}

export interface PopupDismissEvent {
  stopPropagation(): void
}

export interface PopupProps extends ViewProps {
  visible?: boolean;
  anchor?: QtNode;
  placement?: "bottom" | "top" | "right" | "left";
  screenX?: number;
  screenY?: number;
  onDismiss?: (event: PopupDismissEvent) => void;
}

export type PopupSource = PopupProps | Accessor<PopupProps>;

export interface TooltipProps {
  anchor?: QtNode;
  placement?: "bottom" | "top" | "right" | "left";
  hoverDelay?: number;
  hideDelay?: number;
  children?: JSX.Element;
}

export type WindowSource = WindowProps | Accessor<WindowProps>;
export type WindowComposable = (children: Accessor<JSX.Element>) => JSX.Element;
export type WindowConfig = WindowProps;
export type WindowConfigSource = WindowSource;
