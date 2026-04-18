import type { Accessor, JSX } from "solid-js"

import { createElement as createQtElement, spread as spreadQtProps } from "../runtime/reconciler.ts"
import { QT_SOLID_SOURCE_META_PROP } from "../devtools/source-metadata.ts"

import { readQtSourceMetadata } from "./source-meta.ts"
import type {
  TextProps,
  ViewProps,
  WidgetProps,
  WindowProps,
} from "./types.ts"

export function createRuntimeElement(type: string, props: Record<string, unknown>): JSX.Element {
  const node = createQtElement(type)
  spreadQtProps(node, props)
  return node as unknown as JSX.Element
}

export function textLike(children: JSX.Element | undefined) {
  let value: unknown = children
  while (typeof value === "function") {
    value = value()
  }

  if (Array.isArray(value)) {
    if (value.length === 1) {
      value = value[0]
      while (typeof value === "function") {
        value = value()
      }
    } else {
      const parts = value
        .map((part) => {
          let current: unknown = part
          while (typeof current === "function") {
            current = current()
          }
          return typeof current === "string" || typeof current === "number"
            ? String(current)
            : ""
        })
        .filter((part) => part.length > 0)

      return parts.length > 0 ? parts.join("") : undefined
    }
  }

  return typeof value === "string" || typeof value === "number" ? String(value) : undefined
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
    width: getter(() => read().width),
    height: getter(() => read().height),
    minWidth: getter(() => read().minWidth),
    minHeight: getter(() => read().minHeight),
    grow: getter(() => read().flexGrow),
    shrink: getter(() => read().flexShrink),
    enabled: getter(() => read().enabled),
    [QT_SOLID_SOURCE_META_PROP]: getter(() => readQtSourceMetadata(read())),
  })
}

export function widgetProps(props: WidgetProps): Record<string, unknown> {
  return widgetPropsFrom(() => props)
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
    children: getter(() => children()),
  })
}

export function viewProps(props: ViewProps): Record<string, unknown> {
  return viewPropsFrom(() => props)
}

export function windowPropsFrom(
  read: Accessor<WindowProps>,
  children: Accessor<JSX.Element | undefined>,
): Record<string, unknown> {
  return extendProps(viewPropsFrom(read, children), {
    title: getter(() => read().title),
    visible: getter(() => read().visible),
    frameless: getter(() => read().frameless),
    transparentBackground: getter(() => read().transparentBackground),
    alwaysOnTop: getter(() => read().alwaysOnTop),
    onCloseRequested: getter(() => read().onCloseRequested),
  })
}

export function textProps(props: TextProps): Record<string, unknown> {
  return extendProps(widgetProps(props), {
    text: getter(() => props.text ?? textLike(props.children)),
    family: getter(() => props.fontFamily),
    pointSize: getter(() => props.fontPointSize),
    weight: getter(() => props.fontWeight),
    italic: getter(() => props.fontItalic),
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
