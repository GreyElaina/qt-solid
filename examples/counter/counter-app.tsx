import { createMemo, createSignal } from "solid-js"

import {
  createApp,
  createWindow,
  type AppHandle,
  type WindowAllClosedContext,
  type WindowHandle,
} from "@qt-solid/solid"

import { CounterWindowView } from "./counter-view.tsx"

export interface CounterAppOptions {
  onActivate?: () => void
  onWindowAllClosed?: (context: WindowAllClosedContext) => void
}

function createCounterWindow(): WindowHandle {
  const [count, setCount] = createSignal(0)
  const [titleSeed, setTitleSeed] = createSignal("Akashina")
  const [controlsEnabled, setControlsEnabled] = createSignal(true)
  const [horizontal, setHorizontal] = createSignal(false)
  const windowTitle = createMemo(() => titleSeed())

  return createWindow(
    () => ({
      title: windowTitle(),
      width: 480,
      height: 320,
    }),
    () => (
      <CounterWindowView
        count={count()}
        controlsEnabled={controlsEnabled()}
        horizontal={horizontal()}
        onControlsEnabledChange={setControlsEnabled}
        onCountChange={setCount}
        onHorizontalChange={setHorizontal}
        onTitleSeedChange={setTitleSeed}
        titleSeed={titleSeed()}
      />
    ),
  )
}

export function createCounterApp(options: CounterAppOptions = {}): AppHandle {
  return createApp(() => {
    const mainWindow = createCounterWindow()

    return {
      render: () => mainWindow.render(),
      onActivate() {
        mainWindow.open()
        options.onActivate?.()
      },
      onWindowAllClosed: options.onWindowAllClosed,
    }
  })
}
