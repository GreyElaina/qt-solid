import {
  collectQtWidgetPropMetadata,
  defineQtWidgetLibrary,
  defineQtWidgetLibraryEntry,
  propIdForBindingKey,
  registerQtWidgetLibraryEntry,
  type IntrinsicNode,
  type QtControlledPropValue,
  type QtNativeEventEmitter,
  type QtShouldSkipLeafWrite,
  type QtWidgetNativeBridge,
  type QtWidgetLibrary,
  type QtWidgetLibraryEntry,
} from "@qt-solid/core/widget-library"
import type { QtApp, QtHostEvent, QtInitialProp } from "@qt-solid/core/native"

import {
  eventExportSpecs,
  type PropMethodName,
  type QtWidgetBinding,
  qtWidgetEntityMap,
  widgetBindings,
} from "./qt-host.ts"
import type { EventPayloadDecodeKind } from "@qt-solid/core/qt-host.shared"

type PropLeafBinding = QtWidgetBinding["props"][string]
type GeneratedEventExportSpec = (typeof eventExportSpecs)[keyof typeof eventExportSpecs]
type RuntimeEventExportSpec = GeneratedEventExportSpec & { exportName: string }
type EventValue = Extract<QtHostEvent, { type: "listener" }>["values"][number]
type PropMethod = (value: unknown) => void
type AttachMethod = (initialProps: QtInitialProp[]) => void
type PendingMountPropPatch = { prev: unknown; next: unknown }
type PropApplyPhase = "init" | "normal" | "update"
type QtHostWidgetBridge = QtWidgetNativeBridge<typeof qtWidgetEntityMap>
type QtHostWidgetEntityCtor =
  QtHostWidgetBridge["entityMap"][keyof QtHostWidgetBridge["entityMap"]]
type QtHostWidgetEntity = ReturnType<QtHostWidgetEntityCtor["create"]>
type CoreWidgetsLibrary = QtWidgetLibrary<QtHostWidgetBridge>
const IS_DEVELOPMENT =
  typeof process !== "undefined"
    ? process.env.NODE_ENV !== "production"
    : (import.meta as ImportMeta & { env?: { DEV?: boolean } }).env?.DEV === true
const warnedCreateOnlyUpdates = new Set<string>()
const eventExportSpecEntries = Object.entries(eventExportSpecs) as Array<
  [string, GeneratedEventExportSpec]
>
const intrinsicKind: Record<string, string> = Object.fromEntries(
  Object.keys(widgetBindings).map((intrinsicName) => [intrinsicName, intrinsicName]),
)
const eventExportIds: Record<string, number> = Object.fromEntries(
  eventExportSpecEntries.map(([exportName, spec]) => [exportName, spec.exportId]),
)
const eventExportSpecsById = new Map<number, RuntimeEventExportSpec>(
  eventExportSpecEntries.map(([exportName, spec]) => [
    spec.exportId,
    { exportName, ...spec },
  ]),
)
const propMetadata = collectQtWidgetPropMetadata(widgetBindings)
const qtWidgetNativeBridge: QtHostWidgetBridge = {
  entityMap: qtWidgetEntityMap,
}

function resolveLeafValue(
  leaf: PropLeafBinding,
  value: unknown,
): { hasValue: true; value: unknown } | { hasValue: false } {
  if (value == null) {
    if (leaf.reset === undefined) {
      return { hasValue: false }
    }
    return { hasValue: true, value: leaf.reset }
  }

  return { hasValue: true, value }
}

function hasEntityMethod<MethodName extends PropMethodName>(
  entity: QtHostWidgetEntity,
  method: MethodName,
): entity is QtHostWidgetEntity & Record<MethodName, PropMethod> {
  return typeof Reflect.get(entity, method) === "function"
}

function hasEntityAttach(
  entity: QtHostWidgetEntity,
): entity is QtHostWidgetEntity & { __qtAttach: AttachMethod } {
  return typeof Reflect.get(entity, "__qtAttach") === "function"
}

function hasOwnKey<ObjectType extends object>(
  object: ObjectType,
  key: PropertyKey,
): key is keyof ObjectType {
  return Object.hasOwn(object, key)
}

function warnCreateOnlyUpdate(path: string): void {
  if (!IS_DEVELOPMENT || warnedCreateOnlyUpdates.has(path)) {
    return
  }

  warnedCreateOnlyUpdates.add(path)
  console.warn(`Qt prop ${path} is create-only; update ignored after mount`)
}

function leafMethodForPhase(
  leaf: PropLeafBinding,
  phase: PropApplyPhase,
): PropMethodName | undefined {
  if (phase === "init") {
    return leaf.initMethod
  }

  if (phase === "normal") {
    return leaf.initMethod == null ? leaf.method : undefined
  }

  return leaf.method
}

function applyLeafBinding(
  entity: QtHostWidgetEntity,
  key: string,
  path: string,
  leaf: PropLeafBinding,
  prev: unknown,
  next: unknown,
  prevExists: boolean,
  phase: PropApplyPhase,
  shouldSkipLeafWrite?: QtShouldSkipLeafWrite,
): void {
  const method = leafMethodForPhase(leaf, phase)
  if (method == null) {
    if (phase === "update" && leaf.initMethod != null) {
      warnCreateOnlyUpdate(path)
    }
    return
  }

  const nextResolved = resolveLeafValue(leaf, next)
  if (!nextResolved.hasValue) {
    return
  }

  if (prevExists) {
    const prevResolved = resolveLeafValue(leaf, prev)
    if (prevResolved.hasValue && Object.is(prevResolved.value, nextResolved.value)) {
      return
    }
  }

  if (shouldSkipLeafWrite?.(leaf.key, nextResolved.value) === true) {
    return
  }

  if (!hasEntityMethod(entity, method)) {
    throw new TypeError(`Qt prop ${key} has no entity method ${method}`)
  }

  entity[method](leaf.parseValue(nextResolved.value, path))
}

function applyWidgetPropBinding(
  binding: QtWidgetBinding,
  entity: QtHostWidgetEntity,
  key: string,
  prev: unknown,
  next: unknown,
  phase: PropApplyPhase,
  shouldSkipLeafWrite?: QtShouldSkipLeafWrite,
): boolean {
  const leaf = binding.props[key]
  if (!leaf) {
    return false
  }

  applyLeafBinding(
    entity,
    key,
    key,
    leaf,
    prev,
    next,
    prev !== undefined,
    phase,
    shouldSkipLeafWrite,
  )
  return true
}

function createInitialProp(
  key: string,
  path: string,
  leaf: PropLeafBinding,
  next: unknown,
): QtInitialProp | undefined {
  const resolved = resolveLeafValue(leaf, next)
  if (!resolved.hasValue) {
    return undefined
  }

  const value = leaf.parseValue(resolved.value, path)
  switch (leaf.valueKind) {
    case "string":
    case "enum":
      return { key, stringValue: String(value) }
    case "boolean":
      return { key, boolValue: value as boolean }
    case "integer":
      return { key, i32Value: value as number }
    case "number":
      return { key, f64Value: value as number }
  }
}

function createBoundIntrinsicNode(
  bridge: QtHostWidgetBridge,
  app: QtApp,
  intrinsicName: string,
): IntrinsicNode | undefined {
  if (!hasOwnKey(widgetBindings, intrinsicName)) {
    return undefined
  }
  const binding: QtWidgetBinding = widgetBindings[intrinsicName]
  if (!hasOwnKey(bridge.entityMap, intrinsicName)) {
    throw new Error(`Qt native bridge is missing entity for intrinsic ${intrinsicName}`)
  }
  const entityCtor = bridge.entityMap[intrinsicName]
  const entity = entityCtor.create(app)
  const node = entity.node
  let mounted = false
  const pendingProps = new Map<string, PendingMountPropPatch>()

  const flushPendingProps = () => {
    if (mounted) {
      return
    }

    const createKeys = new Set<string>()
    const createProps: QtInitialProp[] = []
    for (const [key, patch] of pendingProps) {
      const leaf = binding.props[key]
      if (!leaf?.create) {
        continue
      }
      const initialProp = createInitialProp(key, key, leaf, patch.next)
      if (!initialProp) {
        continue
      }
      createKeys.add(key)
      createProps.push(initialProp)
    }

    if (createProps.length > 0) {
      if (!hasEntityAttach(entity)) {
        throw new TypeError(`Qt intrinsic ${intrinsicName} is missing __qtAttach`)
      }
      entity.__qtAttach(createProps)
    }

    for (const phase of ["init", "normal"] as const) {
      for (const [key, patch] of pendingProps) {
        if (createKeys.has(key)) {
          continue
        }
        applyWidgetPropBinding(binding, entity, key, patch.prev, patch.next, phase)
      }
    }

    pendingProps.clear()
    mounted = true
  }

  return {
    node,
    propIdForKey(key) {
      return propIdForBindingKey(binding, key)
    },
    applyProp(key, prev, next, shouldSkipLeafWrite) {
      if (!mounted) {
        if (!hasOwnKey(binding.props, key)) {
          return false
        }

        const existing = pendingProps.get(key)
        pendingProps.set(key, {
          prev: existing?.prev ?? prev,
          next,
        })
        return true
      }

      return applyWidgetPropBinding(
        binding,
        entity,
        key,
        prev,
        next,
        "update",
        shouldSkipLeafWrite,
      )
    },
    finalizeMount() {
      flushPendingProps()
    },
  }
}

function findEventValue(values: EventValue[], path: string): EventValue | undefined {
  return values.find((value) => value.path === path)
}

export function readStringEventValue(values: EventValue[], path: string): string {
  const value = findEventValue(values, path)
  if (value?.kindTag !== 1 || value.stringValue == null) {
    throw new TypeError(`Qt event payload ${path || "<value>"} expects string`)
  }

  return value.stringValue
}

export function readBooleanEventValue(values: EventValue[], path: string): boolean {
  const value = findEventValue(values, path)
  if (value?.kindTag !== 2 || value.boolValue == null) {
    throw new TypeError(`Qt event payload ${path || "<value>"} expects boolean`)
  }

  return value.boolValue
}

export function readI32EventValue(values: EventValue[], path: string): number {
  const value = findEventValue(values, path)
  if (value?.kindTag !== 3 || value.i32Value == null) {
    throw new TypeError(`Qt event payload ${path || "<value>"} expects integer`)
  }

  return value.i32Value
}

export function readF64EventValue(values: EventValue[], path: string): number {
  const value = findEventValue(values, path)
  if (value?.kindTag !== 4 || value.f64Value == null) {
    throw new TypeError(`Qt event payload ${path || "<value>"} expects number`)
  }

  return value.f64Value
}

export function readNumberEventValue(values: EventValue[], path: string): number {
  const value = findEventValue(values, path)
  if (value == null) {
    throw new TypeError(`Qt event payload ${path || "<value>"} expects number`)
  }

  if (value.kindTag === 3 && value.i32Value != null) {
    return value.i32Value
  }

  if (value.kindTag === 4 && value.f64Value != null) {
    return value.f64Value
  }

  throw new TypeError(`Qt event payload ${path || "<value>"} expects number`)
}

function readEventValue(
  values: EventValue[],
  path: string,
  kind: EventPayloadDecodeKind,
): unknown {
  switch (kind) {
    case "string":
      return readStringEventValue(values, path)
    case "boolean":
      return readBooleanEventValue(values, path)
    case "number":
      return readNumberEventValue(values, path)
    case "unit":
    case "object":
      throw new TypeError(`Qt event payload ${path || "<value>"} uses unsupported scalar decode ${kind}`)
  }
}

function collectControlledEventValues(event: QtHostEvent): QtControlledPropValue[] {
  if (event.type !== "listener") {
    return []
  }

  const spec = eventExportSpecsById.get(event.listenerId)
  if (!spec || spec.echoes.length === 0) {
    return []
  }

  return spec.echoes.map((echo) => ({
    propKey: echo.propKey,
    value: readEventValue(event.values, echo.valuePath, echo.valueKind),
  }))
}

function isEventExportPropKey(key: string): boolean {
  return Object.hasOwn(eventExportIds, key)
}

function dispatchEventExport(emit: QtNativeEventEmitter, event: QtHostEvent): void {
  if (event.type !== "listener") {
    return
  }

  const spec = eventExportSpecsById.get(event.listenerId)
  if (!spec) {
    return
  }

  switch (spec.payloadKind) {
    case "unit":
      emit(event.nodeId, spec.exportName)
      return
    case "string":
    case "boolean":
    case "number":
      emit(
        event.nodeId,
        spec.exportName,
        readEventValue(event.values, "", spec.payloadKind),
      )
      return
    case "object": {
      const payload: Record<string, unknown> = {}
      for (const field of spec.payloadFields) {
        payload[field.key] = readEventValue(event.values, field.valuePath, field.valueKind)
      }
      emit(event.nodeId, spec.exportName, payload)
      return
    }
  }
}

export const coreWidgetsLibrary = defineQtWidgetLibrary<QtHostWidgetBridge, CoreWidgetsLibrary>({
  name: "@qt-solid/core-widgets",
  intrinsicKind,
  metadata: {
    props: propMetadata,
  },
  eventExportIds,
  collectControlledPropValues: collectControlledEventValues,
  dispatchNativeEvent: dispatchEventExport,
  isEventExportProp: isEventExportPropKey,
  createIntrinsicNode(bridge, app, intrinsicName) {
    return createBoundIntrinsicNode(bridge, app, intrinsicName)
  },
})

export const coreWidgetsLibraryEntry = defineQtWidgetLibraryEntry<
  QtHostWidgetBridge,
  CoreWidgetsLibrary
>({
  library: coreWidgetsLibrary,
  nativeBridge: qtWidgetNativeBridge,
})

export const qtWidgetLibrary: QtWidgetLibrary = coreWidgetsLibrary
export const qtWidgetLibraryEntry: QtWidgetLibraryEntry = coreWidgetsLibraryEntry
export {
  intrinsicKind,
  eventExportIds,
  qtWidgetNativeBridge,
}

export function registerCoreWidgetsLibrary(options: { default?: boolean } = {}): QtWidgetLibraryEntry {
  return registerQtWidgetLibraryEntry(coreWidgetsLibraryEntry, options)
}

export function registerQtSolidWidgetLibrary(
  options: { default?: boolean } = {},
): QtWidgetLibraryEntry {
  return registerCoreWidgetsLibrary(options)
}
