import { createEffect, createRoot, type Accessor } from "solid-js"

export interface QtSolidBinding {
  initQt(onEvent: (error: Error | null, event: string) => void): void | Promise<void>
  setRectangleColor(color: string): void
  setLabelText(text: string): void
  setLabelTextAsync(text: string, delayMs: number): void
}

export interface QtControllerState {
  color: Accessor<string>
  label: Accessor<string>
}

export function mountControllerBridge(
  binding: QtSolidBinding,
  state: QtControllerState,
  onEvent?: (event: string) => void,
): () => void {
  void binding.initQt((error, event) => {
    if (error) {
      throw error
    }

    onEvent?.(event)
  })

  return createRoot((dispose) => {
    createEffect(() => {
      binding.setRectangleColor(state.color())
    })

    createEffect(() => {
      binding.setLabelText(state.label())
    })

    return dispose
  })
}
