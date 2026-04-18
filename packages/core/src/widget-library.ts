import type { QtApp, QtHostEvent, QtNode } from "../native/index.js"

export type QtWidgetKind = string
export type QtControlledPropValue = { propKey: string; value: unknown }
export type QtNativeEventEmitter = (nodeId: number, key: string, ...args: unknown[]) => void
export type QtShouldSkipLeafWrite = (leafKey: string, value: unknown) => boolean

export interface QtWidgetKindMetadata {
  readonly intrinsicName: string
  readonly nativeKind: QtWidgetKind
}

export interface QtWidgetPropMetadata {
  readonly jsName: string
  readonly propId: number
}

export interface QtWidgetBindingLike {
  readonly props: Record<string, { readonly propId?: number; readonly [key: string]: unknown }>
}

export interface QtWidgetEventExportMetadata {
  readonly exportName: string
  readonly exportId: number
}

export interface QtWidgetLibraryMetadata {
  readonly kinds: readonly QtWidgetKindMetadata[]
  readonly props: readonly QtWidgetPropMetadata[]
  readonly eventExports: readonly QtWidgetEventExportMetadata[]
}

export interface QtWidgetEntity {
  readonly node: QtNode
}

export interface QtWidgetEntityCtor<Entity extends QtWidgetEntity = QtWidgetEntity> {
  create(app: QtApp): Entity
}

export interface QtWidgetNativeBridge<
  EntityMap extends Record<string, QtWidgetEntityCtor> = Record<string, QtWidgetEntityCtor>,
> {
  readonly entityMap: EntityMap
}

export interface IntrinsicNode {
  readonly node: QtNode
  propIdForKey(key: string): number | undefined
  finalizeMount(): void
  applyProp(
    key: string,
    prev: unknown,
    next: unknown,
    shouldSkipLeafWrite?: QtShouldSkipLeafWrite,
  ): boolean
}

export interface QtWidgetLibrary<
  Bridge extends QtWidgetNativeBridge = QtWidgetNativeBridge,
> {
  readonly name: string
  readonly metadata: QtWidgetLibraryMetadata
  readonly intrinsicKind: Record<string, QtWidgetKind>
  readonly eventExportIds: Record<string, number>
  collectControlledPropValues(event: QtHostEvent): QtControlledPropValue[]
  dispatchNativeEvent(emit: QtNativeEventEmitter, event: QtHostEvent): void
  isEventExportProp(key: string): boolean
  createIntrinsicNode(
    bridge: Bridge,
    app: QtApp,
    intrinsicName: string,
  ): IntrinsicNode | undefined
}

export interface QtWidgetLibraryEntry<
  Bridge extends QtWidgetNativeBridge = QtWidgetNativeBridge,
  Library extends QtWidgetLibrary<Bridge> = QtWidgetLibrary<Bridge>,
> {
  readonly library: Library
  readonly nativeBridge: Bridge
}

type QtWidgetLibraryDefinition<
  Bridge extends QtWidgetNativeBridge = QtWidgetNativeBridge,
  Library extends QtWidgetLibrary<Bridge> = QtWidgetLibrary<Bridge>,
> = Omit<
  Library,
  "metadata"
> & {
  readonly metadata?: Partial<QtWidgetLibraryMetadata>
}

type QtWidgetLibraryEntryDefinition<
  Bridge extends QtWidgetNativeBridge = QtWidgetNativeBridge,
  Library extends QtWidgetLibrary<Bridge> = QtWidgetLibrary<Bridge>,
> = QtWidgetLibraryEntry<Bridge, Library>
type RegisterQtWidgetLibraryOptions = {
  default?: boolean
  nativeBridge: QtWidgetNativeBridge
}
type RegisterQtWidgetLibraryEntryOptions = {
  default?: boolean
}

function compareStrings(left: string, right: string): number {
  if (left < right) {
    return -1
  }
  if (left > right) {
    return 1
  }
  return 0
}

function compareNumbers(left: number, right: number): number {
  return left - right
}

export function propIdForBindingKey(
  binding: QtWidgetBindingLike,
  key: string,
): number | undefined {
  return binding.props[key]?.propId
}

export function collectQtWidgetPropMetadata(
  bindings: Record<string, QtWidgetBindingLike>,
): readonly QtWidgetPropMetadata[] {
  const props = new Map<string, number>()

  for (const binding of Object.values(bindings)) {
    for (const [key, prop] of Object.entries(binding.props)) {
      if (prop.propId == null) {
        continue
      }
      if (props.has(key)) {
        continue
      }

      props.set(key, prop.propId)
    }
  }

  return Array.from(props, ([jsName, propId]) => ({ jsName, propId }))
}

function deriveLibraryMetadata(definition: QtWidgetLibraryDefinition): QtWidgetLibraryMetadata {
  const kinds = Object.entries(definition.intrinsicKind)
    .map(([intrinsicName, nativeKind]) => ({ intrinsicName, nativeKind }))
    .sort((left, right) => compareStrings(left.intrinsicName, right.intrinsicName))

  const props = (definition.metadata?.props?.length ? definition.metadata.props : [])
    .slice()
    .sort((left, right) => compareNumbers(left.propId, right.propId))

  const eventExports = Object.entries(definition.eventExportIds)
    .map(([exportName, exportId]) => ({ exportName, exportId }))
    .sort((left, right) => compareNumbers(left.exportId, right.exportId))

  return {
    kinds: definition.metadata?.kinds?.length ? definition.metadata.kinds : kinds,
    props,
    eventExports: definition.metadata?.eventExports?.length
      ? definition.metadata.eventExports
      : eventExports,
  }
}

export function defineQtWidgetLibrary<
  Bridge extends QtWidgetNativeBridge,
  Library extends QtWidgetLibrary<Bridge>,
>(
  definition: QtWidgetLibraryDefinition<Bridge, Library>,
): Library {
  return {
    ...definition,
    metadata: deriveLibraryMetadata(definition),
  } as Library
}

export function defineQtWidgetLibraryEntry<
  Bridge extends QtWidgetNativeBridge,
  Library extends QtWidgetLibrary<Bridge>,
>(
  definition: QtWidgetLibraryEntryDefinition<Bridge, Library>,
): QtWidgetLibraryEntry<Bridge, Library> {
  return definition
}

const widgetLibraryEntries = new Map<string, QtWidgetLibraryEntry>()
let defaultLibraryName: string | undefined

export function registerQtWidgetLibrary(
  library: QtWidgetLibrary,
  options: RegisterQtWidgetLibraryOptions,
): QtWidgetLibrary {
  registerQtWidgetLibraryEntry(
    defineQtWidgetLibraryEntry({
      library,
      nativeBridge: options.nativeBridge,
    }),
    { default: options.default },
  )
  return library
}

export function registerQtWidgetLibraryEntry(
  entry: QtWidgetLibraryEntry,
  options: RegisterQtWidgetLibraryEntryOptions = {},
): QtWidgetLibraryEntry {
  for (const [intrinsicName] of Object.entries(entry.library.intrinsicKind)) {
    const existing = Array.from(widgetLibraryEntries.values()).find(
      (candidate) =>
        candidate.library.name !== entry.library.name &&
        Object.hasOwn(candidate.library.intrinsicKind, intrinsicName),
    )
    if (existing) {
      throw new Error(
        `Qt intrinsic ${intrinsicName} is already registered by ${existing.library.name}`,
      )
    }
  }

  widgetLibraryEntries.set(entry.library.name, entry)
  if (options.default ?? defaultLibraryName == null) {
    defaultLibraryName = entry.library.name
  }
  return entry
}

export function registeredQtWidgetLibraries(): readonly QtWidgetLibrary[] {
  return Array.from(widgetLibraryEntries.values(), (entry) => entry.library)
}

export function registeredQtWidgetLibraryEntries(): readonly QtWidgetLibraryEntry[] {
  return Array.from(widgetLibraryEntries.values())
}

export function resolveQtWidgetLibrary(name: string): QtWidgetLibrary {
  return resolveQtWidgetLibraryEntry(name).library
}

export function resolveQtWidgetLibraryEntry(name: string): QtWidgetLibraryEntry {
  const entry = widgetLibraryEntries.get(name)
  if (!entry) {
    throw new Error(`Qt widget library is not registered: ${name}`)
  }
  return entry
}

export function resolveQtWidgetLibraryNativeBridge(name: string): QtWidgetNativeBridge | undefined {
  return widgetLibraryEntries.get(name)?.nativeBridge
}

export function resolveDefaultQtWidgetLibrary(): QtWidgetLibrary {
  return resolveDefaultQtWidgetLibraryEntry().library
}

export function resolveDefaultQtWidgetLibraryEntry(): QtWidgetLibraryEntry {
  if (!defaultLibraryName) {
    throw new Error("Qt widget library registry is empty")
  }

  return resolveQtWidgetLibraryEntry(defaultLibraryName)
}

export function resolveDefaultQtWidgetLibraryNativeBridge(): QtWidgetNativeBridge | undefined {
  return resolveDefaultQtWidgetLibraryEntry().nativeBridge
}

export function resolveQtWidgetLibraryEntryForIntrinsic(intrinsicName: string): QtWidgetLibraryEntry {
  const defaultEntry = defaultLibraryName ? widgetLibraryEntries.get(defaultLibraryName) : undefined
  if (defaultEntry && Object.hasOwn(defaultEntry.library.intrinsicKind, intrinsicName)) {
    return defaultEntry
  }

  let match: QtWidgetLibraryEntry | undefined
  for (const entry of widgetLibraryEntries.values()) {
    if (!Object.hasOwn(entry.library.intrinsicKind, intrinsicName)) {
      continue
    }
    if (!match) {
      match = entry
      continue
    }
    throw new Error(
      `Qt intrinsic ${intrinsicName} is ambiguous across libraries ${match.library.name} and ${entry.library.name}`,
    )
  }

  if (!match) {
    throw new Error(`Qt intrinsic is not registered: ${intrinsicName}`)
  }

  return match
}

export function clearRegisteredQtWidgetLibraries(): void {
  widgetLibraryEntries.clear()
  defaultLibraryName = undefined
}

export function libraryEntryOf(library: QtWidgetLibrary): QtWidgetLibraryEntry | undefined {
  const entry = widgetLibraryEntries.get(library.name)
  if (!entry || entry.library !== library) {
    return undefined
  }
  return entry
}

export function libraryNativeBridgeOf(library: QtWidgetLibrary): QtWidgetNativeBridge | undefined {
  const entry = widgetLibraryEntries.get(library.name)
  if (entry?.library !== library) {
    return undefined
  }
  return entry.nativeBridge
}
