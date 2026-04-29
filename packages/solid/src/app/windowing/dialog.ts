import {
  showOpenFileDialog,
  showSaveFileDialog,
} from "@qt-solid/core/native"
import { fileDialogChannel } from "../../runtime/renderer.ts"

export interface OpenFileDialogOptions {
  windowId: number
  title?: string
  filter?: string
  multiple?: boolean
}

export interface SaveFileDialogOptions {
  windowId: number
  title?: string
  filter?: string
  defaultName?: string
}

export function openFileDialog(options: OpenFileDialogOptions): Promise<string[] | null> {
  const requestId = showOpenFileDialog(
    options.windowId,
    options.title ?? "Open File",
    options.filter ?? null,
    options.multiple ?? false,
  )

  return new Promise((resolve) => {
    const unsubscribe = fileDialogChannel.subscribe((result) => {
      if (result.requestId !== requestId) return
      unsubscribe()
      resolve(result.paths.length > 0 ? result.paths : null)
    })
  })
}

export function saveFileDialog(options: SaveFileDialogOptions): Promise<string | null> {
  const requestId = showSaveFileDialog(
    options.windowId,
    options.title ?? "Save File",
    options.filter ?? null,
    options.defaultName ?? null,
  )

  return new Promise((resolve) => {
    const unsubscribe = fileDialogChannel.subscribe((result) => {
      if (result.requestId !== requestId) return
      unsubscribe()
      resolve(result.paths.length > 0 ? result.paths[0]! : null)
    })
  })
}
