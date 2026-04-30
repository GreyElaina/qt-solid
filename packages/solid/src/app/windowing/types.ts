import type { Accessor, JSX } from "solid-js";
import type {
  ViewIntrinsicProps,
  WindowIntrinsicProps,
} from "../../qt-intrinsics.ts";
import type { ViewProps } from "../types.ts";

export interface WindowProps extends ViewProps {
  title?: WindowIntrinsicProps["title"];
  visible?: WindowIntrinsicProps["visible"];
  frameless?: WindowIntrinsicProps["frameless"];
  transparentBackground?: WindowIntrinsicProps["transparentBackground"];
  alwaysOnTop?: WindowIntrinsicProps["alwaysOnTop"];
  gpu?: WindowIntrinsicProps["gpu"];
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
