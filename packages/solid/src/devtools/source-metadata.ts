import { isAbsolute, resolve } from "node:path"
import { pathToFileURL } from "node:url"

export const QT_SOLID_SOURCE_META_PROP = "__qtSolidSource"

export type QtSolidSourceFrameKind = "user" | "library"

export interface QtSolidSourceMetadata {
  fileName: string
  lineNumber: number
  columnNumber: number
  fileUrl?: string
  projectRootUrl?: string
}

function isOptionalString(value: unknown): value is string | undefined {
  return value == null || typeof value === "string"
}

export function cloneQtSolidSourceMetadata(source: QtSolidSourceMetadata): QtSolidSourceMetadata {
  return {
    fileName: source.fileName,
    lineNumber: source.lineNumber,
    columnNumber: source.columnNumber,
    ...(source.fileUrl ? { fileUrl: source.fileUrl } : {}),
    ...(source.projectRootUrl ? { projectRootUrl: source.projectRootUrl } : {}),
  }
}

export function isQtSolidSourceMetadata(value: unknown): value is QtSolidSourceMetadata {
  if (typeof value !== "object" || value == null) {
    return false
  }

  const candidate = value as Record<string, unknown>
  return typeof candidate.fileName === "string"
    && typeof candidate.lineNumber === "number"
    && Number.isInteger(candidate.lineNumber)
    && typeof candidate.columnNumber === "number"
    && Number.isInteger(candidate.columnNumber)
    && isOptionalString(candidate.fileUrl)
    && isOptionalString(candidate.projectRootUrl)
}

export function formatQtSolidSourceMetadata(source: QtSolidSourceMetadata): string {
  return `${source.fileName}:${source.lineNumber}:${source.columnNumber}`
}

export function qtSolidSourceMetadataUrl(source: QtSolidSourceMetadata): string {
  if (source.fileUrl) {
    return source.fileUrl
  }

  const fileName = isAbsolute(source.fileName) ? source.fileName : resolve(process.cwd(), source.fileName)
  return pathToFileURL(fileName).href
}

export function qtSolidSourceMetadataFrameKind(source: QtSolidSourceMetadata): QtSolidSourceFrameKind {
  const url = qtSolidSourceMetadataUrl(source)
  if (url.includes("/node_modules/")) {
    return "library"
  }

  if (!source.projectRootUrl) {
    return "user"
  }

  return url.startsWith(source.projectRootUrl) ? "user" : "library"
}

export function serializeQtSolidSourceLocation(source: QtSolidSourceMetadata): Record<string, unknown> {
  return {
    source: formatQtSolidSourceMetadata(source),
    sourceFileName: source.fileName,
    sourceLineNumber: source.lineNumber,
    sourceColumnNumber: source.columnNumber,
    sourceUrl: qtSolidSourceMetadataUrl(source),
    frameKind: qtSolidSourceMetadataFrameKind(source),
  }
}

export function sameQtSolidSourceMetadata(
  left: QtSolidSourceMetadata | null,
  right: QtSolidSourceMetadata | null,
): boolean {
  if (left === right) {
    return true
  }

  if (!left || !right) {
    return false
  }

  return left.fileName === right.fileName
    && left.lineNumber === right.lineNumber
    && left.columnNumber === right.columnNumber
    && left.fileUrl === right.fileUrl
    && left.projectRootUrl === right.projectRootUrl
}
