import { createMemo, createSignal } from "solid-js"

import {
  createApp,
  createWindow,
  type AppHandle,
  type WindowAllClosedContext,
  type WindowHandle,
} from "@qt-solid/solid"

import {
  backspaceDisplay,
  clearCalculator,
  createInitialCalculatorState,
  evaluateResult,
  expressionText,
  inputDecimal,
  inputDigit,
  replaceDisplay,
  setOperator,
  toggleSign,
  type CalculatorOperator,
} from "./calculator-logic.ts"
import { CalculatorView } from "./calculator-view.tsx"

export interface CalculatorAppOptions {
  onActivate?: () => void
  onWindowAllClosed?: (context: WindowAllClosedContext) => void
}

function createCalculatorWindow(): WindowHandle {
  const [state, setState] = createSignal(createInitialCalculatorState())
  const windowTitle = createMemo(() => `calculator · ${state().display}`)
  const statusText = createMemo(() => expressionText(state()))

  const pushDigit = (digit: string) => {
    setState((current) => inputDigit(current, digit))
  }

  const applyOperator = (operator: CalculatorOperator) => {
    setState((current) => setOperator(current, operator))
  }

  return createWindow(
    () => ({
      title: windowTitle(),
      width: 360,
      height: 420,
    }),
    () => (
      <CalculatorView
        display={state().display}
        onBackspace={() => setState((current) => backspaceDisplay(current))}
        onClear={() => setState(clearCalculator())}
        onDecimal={() => setState((current) => inputDecimal(current))}
        onDigit={pushDigit}
        onDisplayChange={(value) => setState((current) => replaceDisplay(current, value))}
        onEvaluate={() => setState((current) => evaluateResult(current))}
        onOperator={applyOperator}
        onToggleSign={() => setState((current) => toggleSign(current))}
        statusText={statusText()}
      />
    ),
  )
}

export function createCalculatorApp(options: CalculatorAppOptions = {}): AppHandle {
  return createApp(() => {
    const mainWindow = createCalculatorWindow()

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
