import type { QtApp, QtHostEvent } from "@qt-solid/core"
import type { QtWidgetCapture } from "@qt-solid/core/native"
import type {
  ButtonIntrinsicProps,
  CheckIntrinsicProps,
  DoubleSpinBoxIntrinsicProps,
  GroupIntrinsicProps,
  InputIntrinsicProps,
  LabelIntrinsicProps,
  SliderIntrinsicProps,
  TextIntrinsicProps,
  ViewIntrinsicProps,
  WindowIntrinsicProps,
} from "@qt-solid/core-widgets/qt-intrinsics"
import type { Accessor, JSX } from "solid-js"

export type WidgetGeometry = Pick<ViewIntrinsicProps, "width" | "height" | "minWidth" | "minHeight">
export type WidgetFlex = Pick<ViewIntrinsicProps, "grow" | "shrink">
export type WidgetState = Pick<ViewIntrinsicProps, "enabled">
export type ViewLayout = Pick<
  ViewIntrinsicProps,
  "direction" | "justifyContent" | "alignItems" | "gap" | "padding"
>
export type TextFont = Pick<TextIntrinsicProps, "family" | "pointSize" | "weight" | "italic">
export type WidgetFocus = Pick<InputIntrinsicProps, "focusPolicy" | "autoFocus">
export type TextSelection = Pick<
  InputIntrinsicProps,
  "cursorPosition" | "selectionStart" | "selectionEnd"
>
export type SliderRange = Pick<
  SliderIntrinsicProps,
  "value" | "minimum" | "maximum" | "step" | "pageStep"
>
export type DoubleSpinBoxRange = Pick<
  DoubleSpinBoxIntrinsicProps,
  "value" | "minimum" | "maximum" | "step"
>

export interface WidgetProps {
  width?: WidgetGeometry["width"]
  height?: WidgetGeometry["height"]
  minWidth?: WidgetGeometry["minWidth"]
  minHeight?: WidgetGeometry["minHeight"]
  flexGrow?: WidgetFlex["grow"]
  flexShrink?: WidgetFlex["shrink"]
  enabled?: WidgetState["enabled"]
}

export interface ViewProps extends WidgetProps {
  direction?: ViewLayout["direction"]
  justifyContent?: ViewLayout["justifyContent"]
  alignItems?: ViewLayout["alignItems"]
  gap?: ViewLayout["gap"]
  padding?: ViewLayout["padding"]
  children?: JSX.Element
}

export interface WindowProps extends ViewProps {
  title?: WindowIntrinsicProps["title"]
  visible?: WindowIntrinsicProps["visible"]
  frameless?: WindowIntrinsicProps["frameless"]
  transparentBackground?: WindowIntrinsicProps["transparentBackground"]
  alwaysOnTop?: WindowIntrinsicProps["alwaysOnTop"]
  onCloseRequested?: WindowIntrinsicProps["onCloseRequested"]
}

export interface GroupProps extends ViewProps {
  title?: GroupIntrinsicProps["title"]
}

interface FontFacadeProps {
  fontFamily?: TextFont["family"]
  fontPointSize?: TextFont["pointSize"]
  fontWeight?: TextFont["weight"]
  fontItalic?: TextFont["italic"]
}

interface FocusFacadeProps {
  focusPolicy?: WidgetFocus["focusPolicy"]
  autoFocus?: WidgetFocus["autoFocus"]
}

interface SelectionFacadeProps {
  cursorPosition?: TextSelection["cursorPosition"]
  selectionStart?: TextSelection["selectionStart"]
  selectionEnd?: TextSelection["selectionEnd"]
}

interface SliderRangeFacadeProps {
  value?: SliderRange["value"]
  minimum?: SliderRange["minimum"]
  maximum?: SliderRange["maximum"]
  step?: SliderRange["step"]
  pageStep?: SliderRange["pageStep"]
}

interface DoubleSpinBoxRangeFacadeProps {
  value?: DoubleSpinBoxRange["value"]
  minimum?: DoubleSpinBoxRange["minimum"]
  maximum?: DoubleSpinBoxRange["maximum"]
  step?: DoubleSpinBoxRange["step"]
}

export interface LabelProps extends WidgetProps, FontFacadeProps {
  text?: LabelIntrinsicProps["text"]
  children?: JSX.Element
}

export interface ButtonProps extends WidgetProps, FontFacadeProps {
  text?: ButtonIntrinsicProps["text"]
  children?: JSX.Element
  onClicked?: ButtonIntrinsicProps["onClicked"]
}

export interface InputProps extends WidgetProps, FontFacadeProps, FocusFacadeProps, SelectionFacadeProps {
  text?: InputIntrinsicProps["text"]
  placeholder?: InputIntrinsicProps["placeholder"]
  onChanged?: InputIntrinsicProps["onChanged"]
  onTextChanged?: InputIntrinsicProps["onTextChanged"]
  onCursorPositionChanged?: InputIntrinsicProps["onCursorPositionChanged"]
  onSelectionChanged?: InputIntrinsicProps["onSelectionChanged"]
  onFocusIn?: InputIntrinsicProps["onFocusIn"]
  onFocusOut?: InputIntrinsicProps["onFocusOut"]
}

export interface CheckboxProps extends WidgetProps, FontFacadeProps {
  text?: CheckIntrinsicProps["text"]
  checked?: CheckIntrinsicProps["checked"]
  onToggled?: CheckIntrinsicProps["onToggled"]
}

export interface TextProps extends WidgetProps, FontFacadeProps {
  text?: TextIntrinsicProps["text"]
  children?: JSX.Element
}

export interface SliderProps extends WidgetProps, FocusFacadeProps, SliderRangeFacadeProps {
  onValueChanged?: SliderIntrinsicProps["onValueChanged"]
  onFocusIn?: SliderIntrinsicProps["onFocusIn"]
  onFocusOut?: SliderIntrinsicProps["onFocusOut"]
}

export interface DoubleSpinBoxProps
  extends WidgetProps, FocusFacadeProps, DoubleSpinBoxRangeFacadeProps
{
  onValueChanged?: DoubleSpinBoxIntrinsicProps["onValueChanged"]
  onFocusIn?: DoubleSpinBoxIntrinsicProps["onFocusIn"]
  onFocusOut?: DoubleSpinBoxIntrinsicProps["onFocusOut"]
}

export type WindowSource = WindowProps | Accessor<WindowProps>
export type WindowComposable = (children: Accessor<JSX.Element>) => JSX.Element
export type WindowConfig = WindowProps
export type WindowConfigSource = WindowSource

export interface RenderQtOptions {
  attachNativeEvents?: (handleEvent: (event: QtHostEvent) => void) => void
}

export interface AppMountOptions extends RenderQtOptions {}

export interface AppMount {
  dispose(): void
}

export interface WindowAllClosedContext {
  quit(): void
}

export interface CreateAppOptions {
  render(): JSX.Element
  onActivate?: () => void
  onWindowAllClosed?: (context: WindowAllClosedContext) => void
}

export type AppDefinition = WindowHandle | CreateAppOptions
export type AppFactory = () => AppDefinition

export interface AppHandle {
  mount(app: QtApp, options?: AppMountOptions): AppMount
}

export interface WindowHandle {
  render(): JSX.Element
  renderQt(app: QtApp, options?: RenderQtOptions): () => void
  dispose(): void
  open(): void
  requestRepaint(): void
  requestNextFrame(): void
  frameState(): WindowFrameState
  capture(): QtWidgetCapture
}

export interface WindowFrameState {
  seq: number
  elapsedMs: number
  deltaMs: number
}
