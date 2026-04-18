import type { Component, JSX } from "solid-js"

import { createComponent as createQtComponent } from "../runtime/reconciler.ts"

import {
  createRuntimeElement,
  extendProps,
  getter,
  textLike,
  textProps,
  viewProps,
  widgetProps,
} from "./props.ts"
import type {
  ButtonProps,
  CheckboxProps,
  DoubleSpinBoxProps,
  GroupProps,
  InputProps,
  LabelProps,
  SliderProps,
  TextProps,
  ViewProps,
  WindowProps,
} from "./types.ts"
import { useWindow } from "./window.ts"

export function createIntrinsicElement<Props extends object>(
  type: string,
  props: Props,
): JSX.Element {
  return createRuntimeElement(type, props as Record<string, unknown>)
}

export function defineIntrinsicComponent<Props extends object>(
  type: string,
): Component<Props> {
  return (props) => createIntrinsicElement(type, props)
}

export const Window: Component<WindowProps> = (props) => {
  return useWindow(() => props)(() => props.children)
}

export const Flex: Component<ViewProps> = (props) => {
  return createRuntimeElement("view", viewProps(props))
}

export const View: Component<ViewProps> = Flex

type DirectionLockedViewProps = Omit<ViewProps, "direction">

export const Row: Component<DirectionLockedViewProps> = (props) => {
  return createRuntimeElement(
    "view",
    extendProps(viewProps(props), {
      direction: getter(() => "row"),
    }),
  )
}

export const Column: Component<DirectionLockedViewProps> = (props) => {
  return createRuntimeElement(
    "view",
    extendProps(viewProps(props), {
      direction: getter(() => "column"),
    }),
  )
}

export const Group: Component<GroupProps> = (props) => {
  return createRuntimeElement(
    "group",
    extendProps(viewProps(props), {
      title: getter(() => props.title),
    }),
  )
}

export const Text: Component<TextProps> = (props) => {
  return createRuntimeElement("text", textProps(props))
}

export const Label: Component<LabelProps> = (props) => {
  return createRuntimeElement("label", textProps(props))
}

export const Button: Component<ButtonProps> = (props) => {
  return createRuntimeElement(
    "button",
    extendProps(widgetProps(props), {
      text: getter(() => props.text ?? textLike(props.children)),
      onClicked: getter(() => props.onClicked),
      family: getter(() => props.fontFamily),
      pointSize: getter(() => props.fontPointSize),
      weight: getter(() => props.fontWeight),
      italic: getter(() => props.fontItalic),
    }),
  )
}

export const Input: Component<InputProps> = (props) => {
  return createRuntimeElement(
    "input",
    extendProps(widgetProps(props), {
      text: getter(() => props.text),
      placeholder: getter(() => props.placeholder),
      onChanged: getter(() => props.onChanged),
      onTextChanged: getter(() => props.onTextChanged),
      onCursorPositionChanged: getter(() => props.onCursorPositionChanged),
      onSelectionChanged: getter(() => props.onSelectionChanged),
      onFocusIn: getter(() => props.onFocusIn),
      onFocusOut: getter(() => props.onFocusOut),
      family: getter(() => props.fontFamily),
      pointSize: getter(() => props.fontPointSize),
      weight: getter(() => props.fontWeight),
      italic: getter(() => props.fontItalic),
      focusPolicy: getter(() => props.focusPolicy),
      autoFocus: getter(() => props.autoFocus),
      cursorPosition: getter(() => props.cursorPosition),
      selectionStart: getter(() => props.selectionStart),
      selectionEnd: getter(() => props.selectionEnd),
    }),
  )
}

export const Checkbox: Component<CheckboxProps> = (props) => {
  return createRuntimeElement(
    "check",
    extendProps(widgetProps(props), {
      text: getter(() => props.text),
      checked: getter(() => props.checked),
      onToggled: getter(() => props.onToggled),
      family: getter(() => props.fontFamily),
      pointSize: getter(() => props.fontPointSize),
      weight: getter(() => props.fontWeight),
      italic: getter(() => props.fontItalic),
    }),
  )
}

export const Slider: Component<SliderProps> = (props) => {
  return createRuntimeElement(
    "slider",
    extendProps(widgetProps(props), {
      value: getter(() => props.value),
      minimum: getter(() => props.minimum),
      maximum: getter(() => props.maximum),
      step: getter(() => props.step),
      pageStep: getter(() => props.pageStep),
      focusPolicy: getter(() => props.focusPolicy),
      autoFocus: getter(() => props.autoFocus),
      onValueChanged: getter(() => props.onValueChanged),
      onFocusIn: getter(() => props.onFocusIn),
      onFocusOut: getter(() => props.onFocusOut),
    }),
  )
}

export const DoubleSpinBox: Component<DoubleSpinBoxProps> = (props) => {
  return createRuntimeElement(
    "doubleSpinBox",
    extendProps(widgetProps(props), {
      value: getter(() => props.value),
      minimum: getter(() => props.minimum),
      maximum: getter(() => props.maximum),
      step: getter(() => props.step),
      focusPolicy: getter(() => props.focusPolicy),
      autoFocus: getter(() => props.autoFocus),
      onValueChanged: getter(() => props.onValueChanged),
      onFocusIn: getter(() => props.onFocusIn),
      onFocusOut: getter(() => props.onFocusOut),
    }),
  )
}

export function el<P extends Record<string, unknown>>(
  component: Component<P>,
  props: P,
): JSX.Element {
  return createQtComponent(component, props)
}
