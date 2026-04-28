import { createSignal, onCleanup } from "solid-js"
import { systemColorScheme } from "@qt-solid/core/native"
import { onColorSchemeChange } from "../runtime/renderer.ts"

export type ColorScheme = "light" | "dark" | "unknown"

export function useColorScheme(): () => ColorScheme {
  const initial = (systemColorScheme?.() ?? "unknown") as ColorScheme
  const [scheme, setScheme] = createSignal<ColorScheme>(initial)

  const unsubscribe = onColorSchemeChange((s) => {
    setScheme(s as ColorScheme)
  })
  onCleanup(unsubscribe)

  return scheme
}
