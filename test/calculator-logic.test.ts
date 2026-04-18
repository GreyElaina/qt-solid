import { describe, expect, it } from "vitest"

import {
  backspaceDisplay,
  createInitialCalculatorState,
  evaluateResult,
  inputDecimal,
  inputDigit,
  replaceDisplay,
  sanitizeCalculatorInput,
  setOperator,
  toggleSign,
} from "../examples/calculator/calculator-logic"

describe("calculator example logic", () => {
  it("chains immediate operations through pending operator", () => {
    let state = createInitialCalculatorState()
    state = inputDigit(state, "1")
    state = inputDigit(state, "0")
    state = setOperator(state, "+")
    state = inputDigit(state, "5")
    state = setOperator(state, "-")
    state = inputDigit(state, "3")
    state = evaluateResult(state)

    expect(state.display).toBe("12")
    expect(state.pendingOperator).toBeNull()
    expect(state.storedValue).toBeNull()
  })

  it("enters error state on division by zero and resets on next digit", () => {
    let state = createInitialCalculatorState()
    state = inputDigit(state, "8")
    state = setOperator(state, "÷")
    state = inputDigit(state, "0")
    state = evaluateResult(state)

    expect(state.display).toBe("Error")
    expect(state.error).toBe("division by zero")

    state = inputDigit(state, "7")
    expect(state.display).toBe("7")
    expect(state.error).toBeNull()
  })

  it("sanitizes manual input and preserves decimal entry", () => {
    expect(sanitizeCalculatorInput("--12..34abc")).toBe("-12.34")

    let state = createInitialCalculatorState()
    state = replaceDisplay(state, "12..34abc")
    expect(state.display).toBe("12.34")

    state = inputDecimal(createInitialCalculatorState())
    expect(state.display).toBe("0.")
  })

  it("supports backspace and sign toggle", () => {
    let state = createInitialCalculatorState()
    state = replaceDisplay(state, "123")
    state = backspaceDisplay(state)
    state = toggleSign(state)

    expect(state.display).toBe("-12")
  })
})
