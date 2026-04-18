export type PropValueKind = "string" | "boolean" | "integer" | "number" | "enum"
export type EventPayloadDecodeKind = "unit" | "string" | "boolean" | "number" | "object"

export type EventExportEchoSpec = {
  propKey: string
  valueKind: EventPayloadDecodeKind
  valuePath: string
}

export type EventPayloadFieldSpec = {
  key: string
  valueKind: EventPayloadDecodeKind
  valuePath: string
}

export type EventExportSpec = {
  exportId: number
  payloadKind: EventPayloadDecodeKind
  payloadFields: readonly EventPayloadFieldSpec[]
  echoes: readonly EventExportEchoSpec[]
}

export type PropLeafBinding<MethodName extends string = string> = {
  key: string
  propId?: number
  create?: boolean
  valueKind: PropValueKind
  reset?: unknown
  method?: MethodName
  initMethod?: MethodName
  parseValue(value: unknown, key: string): unknown
}

export interface QtWidgetBinding<MethodName extends string = string> {
  readonly props: Record<string, PropLeafBinding<MethodName>>
}

export function asString(value: unknown, key: string): string {
  if (typeof value === "string") {
    return value
  }

  throw new TypeError(`Qt prop ${key} expects string`)
}

export function asBoolean(value: unknown, key: string): boolean {
  if (typeof value === "boolean") {
    return value
  }

  throw new TypeError(`Qt prop ${key} expects boolean`)
}

export function asI32(value: unknown, key: string): number {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    throw new TypeError(`Qt prop ${key} expects integer`)
  }

  if (value < -2147483648 || value > 2147483647) {
    throw new RangeError(`Qt prop ${key} exceeds i32 range`)
  }

  return value
}

export function asNonNegativeI32(value: unknown, key: string): number {
  const parsed = asI32(value, key)
  if (parsed < 0) {
    throw new RangeError(`Qt prop ${key} must be non-negative`)
  }

  return parsed
}

export function asF64(value: unknown, key: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new TypeError(`Qt prop ${key} expects number`)
  }

  return value
}

export function asNonNegativeF64(value: unknown, key: string): number {
  const parsed = asF64(value, key)
  if (parsed < 0) {
    throw new RangeError(`Qt prop ${key} must be non-negative`)
  }

  return parsed
}

export function createEnumValueParser<T>(values: readonly string[]) {
  return (value: unknown, key: string): T => {
    if (typeof value !== "string") {
      throw new TypeError(`Qt prop ${key} expects string`)
    }

    const index = values.indexOf(value)
    if (index === -1) {
      throw new TypeError(`Unsupported Qt ${key}: ${value}`)
    }

    return value as T
  }
}
