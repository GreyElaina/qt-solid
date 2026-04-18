import type { Accessor } from "solid-js"

import { QT_SOLID_SOURCE_META_PROP, type QtSolidSourceMetadata } from "../devtools/source-metadata.ts"

export function readQtSourceMetadata(input: unknown): QtSolidSourceMetadata | undefined {
  if (typeof input !== "object" || input == null) {
    return undefined
  }

  return (input as Record<string, unknown>)[QT_SOLID_SOURCE_META_PROP] as QtSolidSourceMetadata | undefined
}

function withSourceMeta<T>(input: T, source: QtSolidSourceMetadata): T {
  if (typeof input !== "object" || input == null) {
    return input
  }

  const clone = Object.defineProperties({}, Object.getOwnPropertyDescriptors(input)) as T & Record<string, unknown>
  Object.defineProperty(clone, QT_SOLID_SOURCE_META_PROP, {
    value: source,
    enumerable: false,
    configurable: true,
  })
  return clone as T
}

export function withQtSourceMeta<T>(input: T, source: QtSolidSourceMetadata): T {
  if (typeof input === "function") {
    const read = input as Accessor<unknown>
    return (() => {
      const value = read()
      const existing = readQtSourceMetadata(value)
      if (existing || value == null) {
        return value
      }

      return withSourceMeta(value, source)
    }) as T
  }

  const existing = readQtSourceMetadata(input)
  if (existing) {
    return input
  }

  return withSourceMeta(input, source)
}
