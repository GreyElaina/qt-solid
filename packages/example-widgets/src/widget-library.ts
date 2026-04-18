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
import type { QtApp } from "@qt-solid/core/native"

import {
  eventExportSpecs,
  type PropMethodName,
  type QtWidgetBinding,
  qtWidgetEntityMap,
  widgetBindings,
} from "./qt-host.ts"

type PropLeafBinding = QtWidgetBinding["props"][string]
type PropMethod = (value: unknown) => void
type PendingMountPropPatch = { prev: unknown; next: unknown }
type PropApplyPhase = "init" | "normal" | "update"
type ExampleWidgetsBridge = QtWidgetNativeBridge<typeof qtWidgetEntityMap>
type ExampleWidgetEntityCtor =
  ExampleWidgetsBridge["entityMap"][keyof ExampleWidgetsBridge["entityMap"]]
type ExampleWidgetEntity = ReturnType<ExampleWidgetEntityCtor["create"]>
type ExampleWidgetsLibrary = QtWidgetLibrary<ExampleWidgetsBridge>
const IS_DEVELOPMENT =
  typeof process !== "undefined"
    ? process.env.NODE_ENV !== "production"
    : (import.meta as ImportMeta & { env?: { DEV?: boolean } }).env?.DEV === true
const warnedCreateOnlyUpdates = new Set<string>()
const eventExportSpecEntries = Object.entries(eventExportSpecs) as Array<
  [string, { exportId: number }]
>
const intrinsicKind: Record<string, string> = Object.fromEntries(
  Object.keys(widgetBindings).map((intrinsicName) => [intrinsicName, intrinsicName]),
)
const eventExportIds: Record<string, number> = Object.fromEntries(
  eventExportSpecEntries.map(([exportName, spec]) => [exportName, spec.exportId]),
)
const propMetadata = collectQtWidgetPropMetadata(widgetBindings)
const qtWidgetNativeBridge: ExampleWidgetsBridge = {
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
  entity: ExampleWidgetEntity,
  method: MethodName,
): entity is ExampleWidgetEntity & Record<MethodName, PropMethod> {
  return typeof Reflect.get(entity, method) === "function"
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
  entity: ExampleWidgetEntity,
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
  entity: ExampleWidgetEntity,
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

function createBoundIntrinsicNode(
  bridge: ExampleWidgetsBridge,
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

    for (const phase of ["init", "normal"] as const) {
      for (const [key, patch] of pendingProps) {
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

export const exampleWidgetsLibrary: ExampleWidgetsLibrary = defineQtWidgetLibrary({
  name: "@qt-solid/example-widgets",
  intrinsicKind,
  metadata: {
    props: propMetadata,
  },
  eventExportIds,
  collectControlledPropValues(_event): QtControlledPropValue[] {
    return []
  },
  dispatchNativeEvent(_emit: QtNativeEventEmitter, _event): void {},
  isEventExportProp(key) {
    return Object.hasOwn(eventExportIds, key)
  },
  createIntrinsicNode(bridge, app, intrinsicName) {
    return createBoundIntrinsicNode(bridge, app, intrinsicName)
  },
})

export const exampleWidgetsLibraryEntry: QtWidgetLibraryEntry<
  ExampleWidgetsBridge,
  ExampleWidgetsLibrary
> = defineQtWidgetLibraryEntry({
  library: exampleWidgetsLibrary,
  nativeBridge: qtWidgetNativeBridge,
})

export const qtWidgetLibraryEntry: QtWidgetLibraryEntry = exampleWidgetsLibraryEntry
export {
  intrinsicKind,
  eventExportIds,
  qtWidgetNativeBridge,
}

export function registerExampleWidgetsLibrary(options: { default?: boolean } = {}) {
  registerQtWidgetLibraryEntry(exampleWidgetsLibraryEntry, options)
  return exampleWidgetsLibrary
}
