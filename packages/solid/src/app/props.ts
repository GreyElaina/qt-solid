import type { Accessor, JSX } from "solid-js"

import { createElement as createQtElement, spread as spreadQtProps } from "../runtime/renderer.ts"
import { QT_SOLID_SOURCE_META_PROP } from "../devtools/source-metadata.ts"

import { readQtSourceMetadata } from "./source-meta.ts"
import type {
  ViewProps,
  WidgetProps,
} from "./types.ts"
import type { WindowProps } from "./windowing/types.ts"

export function createRuntimeElement(type: string, props: Record<string, unknown>): JSX.Element {
  const node = createQtElement(type)
  spreadQtProps(node, props)
  return node
}

export function toAccessor<T>(value: T | Accessor<T>): Accessor<T> {
  return typeof value === "function" ? (value as Accessor<T>) : () => value
}

export function getter(read: () => unknown): PropertyDescriptor {
  return {
    enumerable: true,
    get: read,
  }
}

export function widgetPropsFrom(read: Accessor<WidgetProps>): Record<string, unknown> {
  return Object.defineProperties({}, {
    ref: getter(() => read().ref),
    width: getter(() => read().width),
    height: getter(() => read().height),
    minWidth: getter(() => read().minWidth),
    minHeight: getter(() => read().minHeight),
    maxWidth: getter(() => (read() as any).maxWidth),
    maxHeight: getter(() => (read() as any).maxHeight),
    aspectRatio: getter(() => (read() as any).aspectRatio),
    grow: getter(() => read().flexGrow),
    shrink: getter(() => read().flexShrink),
    basis: getter(() => (read() as any).flexBasis),
    alignSelf: getter(() => (read() as any).alignSelf),
    margin: getter(() => (read() as any).margin),
    enabled: getter(() => read().enabled),
    hidden: getter(() => (read() as any).hidden),
    onHoverEnter: getter(() => read().onHoverEnter),
    onHoverLeave: getter(() => read().onHoverLeave),
    [QT_SOLID_SOURCE_META_PROP]: getter(() => readQtSourceMetadata(read())),
  })
}

export function viewPropsFrom(
  read: Accessor<ViewProps>,
  children: Accessor<JSX.Element | undefined> = () => read().children,
): Record<string, unknown> {
  return extendProps(widgetPropsFrom(read), {
    direction: getter(() => read().direction),
    justifyContent: getter(() => read().justifyContent),
    alignItems: getter(() => read().alignItems),
    gap: getter(() => read().gap),
    padding: getter(() => read().padding),
    wrap: getter(() => read().wrap),
    backgroundColor: getter(() => read().backgroundColor),
    children: getter(() => children()),
  })
}

export function windowPropsFrom(
  read: Accessor<WindowProps>,
): Record<string, unknown> {
  return Object.defineProperties({}, {
    width: getter(() => read().width),
    height: getter(() => read().height),
    title: getter(() => read().title),
    visible: getter(() => read().visible),
    frameless: getter(() => read().frameless),
    transparentBackground: getter(() => read().transparentBackground),
    alwaysOnTop: getter(() => read().alwaysOnTop),
    onCloseRequested: getter(() => read().onCloseRequested),
    [QT_SOLID_SOURCE_META_PROP]: getter(() => readQtSourceMetadata(read())),
  })
}

export function tooltipPropsFrom(
  children: Accessor<JSX.Element | undefined>,
): Record<string, unknown> {
  return extendProps(viewPropsFrom(() => ({}), children), {
    windowKind: getter(() => 2),
    transparentBackground: getter(() => true),
  })
}

export function extendProps(
  base: Record<string, unknown>,
  extra: Record<string, PropertyDescriptor>,
): Record<string, unknown> {
  return Object.defineProperties({}, {
    ...Object.getOwnPropertyDescriptors(base),
    ...extra,
  })
}
