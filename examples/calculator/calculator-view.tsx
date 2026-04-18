import type { Component } from "solid-js"

import {
  Button,
  Column,
  Group,
  Input,
  Row,
  Text,
} from "@qt-solid/solid"

import type { CalculatorOperator } from "./calculator-logic.ts"

export interface CalculatorViewProps {
  display: string
  onBackspace(): void
  onClear(): void
  onDecimal(): void
  onDigit(digit: string): void
  onDisplayChange(value: string): void
  onEvaluate(): void
  onOperator(operator: CalculatorOperator): void
  onToggleSign(): void
  statusText: string
}

interface CalculatorButtonProps {
  label: string
  onClicked: () => void
}

const CalculatorButton: Component<CalculatorButtonProps> = (props) => {
  return (
    <Button minWidth={64} minHeight={44} onClicked={props.onClicked}>
      {props.label}
    </Button>
  )
}

export const CalculatorView: Component<CalculatorViewProps> = (props) => {
  return (
    <Column gap={12} padding={16}>
      <Group title="Display">
        <Column gap={8} padding={8}>
          <Text>{props.statusText}</Text>
          <Input text={props.display} placeholder="0" onChanged={props.onDisplayChange} />
        </Column>
      </Group>

      <Group title="Keypad">
        <Column gap={8} padding={8}>
          <Row gap={8}>
            <CalculatorButton label="C" onClicked={props.onClear} />
            <CalculatorButton label="⌫" onClicked={props.onBackspace} />
            <CalculatorButton label="±" onClicked={props.onToggleSign} />
            <CalculatorButton label="÷" onClicked={() => props.onOperator("÷")} />
          </Row>
          <Row gap={12}>
            <CalculatorButton label="7" onClicked={() => props.onDigit("7")} />
            <CalculatorButton label="8" onClicked={() => props.onDigit("8")} />
            <CalculatorButton label="9" onClicked={() => props.onDigit("9")} />
            <CalculatorButton label="×" onClicked={() => props.onOperator("×")} />
          </Row>
          <Row gap={8}>
            <CalculatorButton label="4" onClicked={() => props.onDigit("4")} />
            <CalculatorButton label="5" onClicked={() => props.onDigit("5")} />
            <CalculatorButton label="6" onClicked={() => props.onDigit("6")} />
            <CalculatorButton label="-" onClicked={() => props.onOperator("-")} />
          </Row>
          <Row gap={8}>
            <CalculatorButton label="1" onClicked={() => props.onDigit("1")} />
            <CalculatorButton label="2" onClicked={() => props.onDigit("2")} />
            <CalculatorButton label="3" onClicked={() => props.onDigit("3")} />
            <CalculatorButton label="+" onClicked={() => props.onOperator("+")} />
          </Row>
          <Row gap={8}>
            <CalculatorButton label="0" onClicked={() => props.onDigit("0")} />
            <CalculatorButton label="." onClicked={props.onDecimal} />
            <CalculatorButton label="=" onClicked={props.onEvaluate} />
          </Row>
        </Column>
      </Group>
    </Column>
  )
}
