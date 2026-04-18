export type CalculatorOperator = "+" | "-" | "×" | "÷"

export interface CalculatorState {
  display: string
  storedValue: number | null
  pendingOperator: CalculatorOperator | null
  waitingForNextInput: boolean
  error: string | null
}

export function createInitialCalculatorState(): CalculatorState {
  return {
    display: "0",
    storedValue: null,
    pendingOperator: null,
    waitingForNextInput: false,
    error: null,
  }
}

function formatValue(value: number): string {
  if (Object.is(value, -0)) {
    return "0"
  }

  return value.toString()
}

function parseDisplay(display: string): number {
  if (display === "" || display === "-" || display === "." || display === "-.") {
    return 0
  }

  return Number(display)
}

function errorState(message: string): CalculatorState {
  return {
    display: "Error",
    storedValue: null,
    pendingOperator: null,
    waitingForNextInput: false,
    error: message,
  }
}

function applyOperator(left: number, right: number, operator: CalculatorOperator): number | null {
  switch (operator) {
    case "+":
      return left + right
    case "-":
      return left - right
    case "×":
      return left * right
    case "÷":
      return right === 0 ? null : left / right
  }
}

export function sanitizeCalculatorInput(value: string): string {
  let next = ""
  let seenDecimal = false

  for (const char of value) {
    if (char >= "0" && char <= "9") {
      next += char
      continue
    }

    if (char === "-" && next.length === 0) {
      next = "-"
      continue
    }

    if (char === "." && !seenDecimal) {
      if (next === "") {
        next = "0"
      } else if (next === "-") {
        next = "-0"
      }

      next += "."
      seenDecimal = true
    }
  }

  if (next === "" || next === "-") {
    return "0"
  }

  return next
}

export function replaceDisplay(state: CalculatorState, value: string): CalculatorState {
  return {
    ...state,
    display: sanitizeCalculatorInput(value),
    waitingForNextInput: false,
    error: null,
  }
}

export function inputDigit(state: CalculatorState, digit: string): CalculatorState {
  if (state.error) {
    return {
      ...createInitialCalculatorState(),
      display: digit,
    }
  }

  if (state.waitingForNextInput) {
    return {
      ...state,
      display: digit,
      waitingForNextInput: false,
      error: null,
    }
  }

  if (state.display === "0") {
    return {
      ...state,
      display: digit,
      error: null,
    }
  }

  if (state.display === "-0") {
    return {
      ...state,
      display: `-${digit}`,
      error: null,
    }
  }

  return {
    ...state,
    display: `${state.display}${digit}`,
    error: null,
  }
}

export function inputDecimal(state: CalculatorState): CalculatorState {
  if (state.error) {
    return {
      ...createInitialCalculatorState(),
      display: "0.",
    }
  }

  if (state.waitingForNextInput) {
    return {
      ...state,
      display: "0.",
      waitingForNextInput: false,
      error: null,
    }
  }

  if (state.display.includes(".")) {
    return state
  }

  return {
    ...state,
    display: `${state.display}.`,
    error: null,
  }
}

export function clearCalculator(): CalculatorState {
  return createInitialCalculatorState()
}

export function backspaceDisplay(state: CalculatorState): CalculatorState {
  if (state.error) {
    return createInitialCalculatorState()
  }

  if (state.waitingForNextInput) {
    return {
      ...state,
      display: "0",
      waitingForNextInput: false,
      error: null,
    }
  }

  const trimmed = state.display.slice(0, -1)
  if (trimmed === "" || trimmed === "-") {
    return {
      ...state,
      display: "0",
      error: null,
    }
  }

  return {
    ...state,
    display: trimmed,
    error: null,
  }
}

export function toggleSign(state: CalculatorState): CalculatorState {
  if (state.error) {
    return createInitialCalculatorState()
  }

  if (state.display === "0" || state.display === "0.") {
    return state
  }

  return {
    ...state,
    display: state.display.startsWith("-") ? state.display.slice(1) : `-${state.display}`,
    error: null,
  }
}

export function setOperator(state: CalculatorState, operator: CalculatorOperator): CalculatorState {
  if (state.error) {
    return createInitialCalculatorState()
  }

  const currentValue = parseDisplay(state.display)

  if (state.pendingOperator && state.storedValue != null && !state.waitingForNextInput) {
    const result = applyOperator(state.storedValue, currentValue, state.pendingOperator)
    if (result == null) {
      return errorState("division by zero")
    }

    return {
      display: formatValue(result),
      storedValue: result,
      pendingOperator: operator,
      waitingForNextInput: true,
      error: null,
    }
  }

  return {
    ...state,
    storedValue: currentValue,
    pendingOperator: operator,
    waitingForNextInput: true,
    error: null,
  }
}

export function evaluateResult(state: CalculatorState): CalculatorState {
  if (state.error || state.pendingOperator == null || state.storedValue == null) {
    return state
  }

  const currentValue = parseDisplay(state.display)
  const result = applyOperator(state.storedValue, currentValue, state.pendingOperator)
  if (result == null) {
    return errorState("division by zero")
  }

  return {
    display: formatValue(result),
    storedValue: null,
    pendingOperator: null,
    waitingForNextInput: true,
    error: null,
  }
}

export function expressionText(state: CalculatorState): string {
  if (state.error) {
    return state.error
  }

  if (state.pendingOperator != null && state.storedValue != null) {
    if (state.waitingForNextInput) {
      return `${formatValue(state.storedValue)} ${state.pendingOperator}`
    }

    return `${formatValue(state.storedValue)} ${state.pendingOperator} ${state.display}`
  }

  return "ready"
}
