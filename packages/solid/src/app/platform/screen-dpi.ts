import { createSignal, onCleanup } from "solid-js"
import { screenDpiInfo } from "@qt-solid/core/native"
import { onScreenDpiChange } from "../../runtime/renderer.ts"

export interface ScreenDpi {
  dpiX: number
  dpiY: number
  devicePixelRatio: number
}

export function useScreenDpi(windowId: number): () => ScreenDpi {
  const initial = screenDpiInfo?.(windowId)
  const [dpi, setDpi] = createSignal<ScreenDpi>(
    initial
      ? { dpiX: initial.dpiX, dpiY: initial.dpiY, devicePixelRatio: initial.devicePixelRatio }
      : { dpiX: 96, dpiY: 96, devicePixelRatio: 1 },
  )

  const unsubscribe = onScreenDpiChange(() => {
    const info = screenDpiInfo?.(windowId)
    if (info) {
      setDpi({ dpiX: info.dpiX, dpiY: info.dpiY, devicePixelRatio: info.devicePixelRatio })
    }
  })
  onCleanup(unsubscribe)

  return dpi
}
