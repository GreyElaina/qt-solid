import {
  clipboardClear,
  clipboardFormats,
  clipboardGet,
  clipboardGetText,
  clipboardHasText,
  clipboardSet,
  clipboardSetText,
} from "@qt-solid/core/native"

export interface ClipboardEntry {
  mime: string
  data: Buffer
}

export interface UseClipboardResult {
  getText: () => string
  setText: (text: string) => void
  hasText: () => boolean
  formats: () => string[]
  get: (mime: string) => Buffer
  set: (entries: ClipboardEntry[]) => void
  clear: () => void
}

export function useClipboard(): UseClipboardResult {
  return {
    getText: clipboardGetText,
    setText: clipboardSetText,
    hasText: clipboardHasText,
    formats: clipboardFormats,
    get: clipboardGet,
    set: clipboardSet,
    clear: clipboardClear,
  }
}
