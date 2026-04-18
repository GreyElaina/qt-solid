import {
  cloneQtSolidSourceMetadata,
  formatQtSolidSourceMetadata,
  isQtSolidSourceMetadata,
  QT_SOLID_SOURCE_META_PROP,
  qtSolidSourceMetadataUrl,
  sameQtSolidSourceMetadata,
  serializeQtSolidSourceLocation,
  type QtSolidSourceMetadata,
} from "./source-metadata.ts"

export interface QtSolidOwnerFrame {
  componentName: string
  source: QtSolidSourceMetadata | null
}

export interface QtSolidOwnerMetadata {
  ownerStack: QtSolidOwnerFrame[]
}

export type QtSolidCreationFrameRole = "node" | "owner"

function cloneQtSolidOwnerFrame(frame: QtSolidOwnerFrame): QtSolidOwnerFrame {
  return {
    componentName: frame.componentName,
    source: frame.source ? cloneQtSolidSourceMetadata(frame.source) : null,
  }
}

function componentNameFor(value: unknown): string {
  if (typeof value !== "function") {
    return "anonymous"
  }

  const candidate = value as { displayName?: unknown; name?: unknown }
  if (typeof candidate.displayName === "string" && candidate.displayName.length > 0) {
    return candidate.displayName
  }

  if (typeof candidate.name === "string" && candidate.name.length > 0) {
    return candidate.name
  }

  return "anonymous"
}

function sourceMetadataForProps(props: unknown): QtSolidSourceMetadata | null {
  if (typeof props !== "object" || props == null) {
    return null
  }

  const candidate = (props as Record<string, unknown>)[QT_SOLID_SOURCE_META_PROP]
  return isQtSolidSourceMetadata(candidate) ? cloneQtSolidSourceMetadata(candidate) : null
}

const qtSolidOwnerStack: QtSolidOwnerFrame[] = []

export function withQtOwnerFrame<T>(component: unknown, props: unknown, render: () => T): T {
  const source = sourceMetadataForProps(props)
  if (!source) {
    return render()
  }

  qtSolidOwnerStack.push({
    componentName: componentNameFor(component),
    source,
  })

  try {
    return render()
  } finally {
    qtSolidOwnerStack.pop()
  }
}

export function currentQtSolidOwnerMetadata(): QtSolidOwnerMetadata | null {
  if (qtSolidOwnerStack.length === 0) {
    return null
  }

  return {
    ownerStack: qtSolidOwnerStack.map(cloneQtSolidOwnerFrame),
  }
}

export function qtSolidOwnerComponentName(owner: QtSolidOwnerMetadata | null): string {
  return owner?.ownerStack[owner.ownerStack.length - 1]?.componentName ?? ""
}

export function formatQtSolidOwnerPath(owner: QtSolidOwnerMetadata): string {
  return owner.ownerStack.map((frame) => frame.componentName).join(" > ")
}

export function serializeQtSolidOwnerStack(owner: QtSolidOwnerMetadata): Array<Record<string, unknown>> {
  return owner.ownerStack.map((frame) => ({
    componentName: frame.componentName,
    ...(frame.source ? serializeQtSolidSourceLocation(frame.source) : {
      source: "",
      sourceFileName: "",
      sourceLineNumber: 0,
      sourceColumnNumber: 0,
      sourceUrl: "",
      frameKind: "library",
    }),
  }))
}

function qtSolidCreationFrame(
  functionName: string,
  source: QtSolidSourceMetadata,
  frameRole: QtSolidCreationFrameRole,
): Record<string, unknown> {
  return {
    functionName,
    scriptId: "",
    url: qtSolidSourceMetadataUrl(source),
    lineNumber: Math.max(0, source.lineNumber - 1),
    columnNumber: Math.max(0, source.columnNumber - 1),
    ...serializeQtSolidSourceLocation(source),
    frameRole,
  }
}

function qtSolidNodeFrameName(kind: string): string {
  return kind === "#text" ? "text" : kind
}

export function qtSolidCreationFrames(
  owner: QtSolidOwnerMetadata | null,
  source: QtSolidSourceMetadata | null,
  kind: string,
): Array<Record<string, unknown>> {
  const callFrames: Array<Record<string, unknown>> = []
  const leafOwnerFrame = owner?.ownerStack[owner.ownerStack.length - 1] ?? null

  if (source && !sameQtSolidSourceMetadata(source, leafOwnerFrame?.source ?? null)) {
    callFrames.push(qtSolidCreationFrame(leafOwnerFrame?.componentName ?? qtSolidNodeFrameName(kind), source, "node"))
  }

  if (owner) {
    for (let index = owner.ownerStack.length - 1; index >= 0; index -= 1) {
      const frame = owner.ownerStack[index]
      if (!frame?.source) {
        continue
      }

      callFrames.push(qtSolidCreationFrame(frame.componentName, frame.source, "owner"))
    }
  }

  return callFrames
}

export function qtSolidCreationStackTrace(
  owner: QtSolidOwnerMetadata | null,
  source: QtSolidSourceMetadata | null,
  kind: string,
): Record<string, unknown> | null {
  const callFrames = qtSolidCreationFrames(owner, source, kind)
  if (callFrames.length === 0) {
    return null
  }

  return {
    description: "Qt Solid node creation",
    callFrames: callFrames.map((frame) => ({
      functionName: frame.functionName,
      scriptId: frame.scriptId,
      url: frame.url,
      lineNumber: frame.lineNumber,
      columnNumber: frame.columnNumber,
    })),
  }
}

export function sameQtSolidOwnerMetadata(
  left: QtSolidOwnerMetadata | null,
  right: QtSolidOwnerMetadata | null,
): boolean {
  if (left === right) {
    return true
  }

  if (!left || !right) {
    return false
  }

  if (left.ownerStack.length !== right.ownerStack.length) {
    return false
  }

  for (let index = 0; index < left.ownerStack.length; index += 1) {
    const leftFrame = left.ownerStack[index]!
    const rightFrame = right.ownerStack[index]!
    if (leftFrame.componentName !== rightFrame.componentName) {
      return false
    }

    if (!sameQtSolidSourceMetadata(leftFrame.source, rightFrame.source)) {
      return false
    }
  }

  return true
}
