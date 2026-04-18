import type { Component } from "solid-js"

import {
  Button,
  Checkbox,
  Column,
  Group,
  Input,
  Row,
  Text,
  View,
} from "@qt-solid/solid"

export interface CounterWindowViewProps {
  controlsEnabled: boolean
  count: number
  horizontal: boolean
  onControlsEnabledChange(checked: boolean): void
  onCountChange(nextValue: number | ((value: number) => number)): void
  onHorizontalChange(checked: boolean): void
  onTitleSeedChange(value: string): void
  titleSeed: string
}

export const CounterWindowView: Component<CounterWindowViewProps> = (props) => {
  return (
    <Column gap={12} padding={16}>
      <Text>{`qt-solid counter · count ${props.count}`}</Text>

      <Group title="Window">
        <Column gap={8} padding={8}>
          <Text>window title seed</Text>
          <Input
            text={props.titleSeed}
            placeholder="window title"
            onChanged={props.onTitleSeedChange}
          />
        </Column>
      </Group>

      <Group title="Controls">
        <Column gap={8} padding={8}>
          <Checkbox
            text="buttons enabled"
            checked={props.controlsEnabled}
            onToggled={props.onControlsEnabledChange}
          />
          <Checkbox
            text="horizontal counter layout"
            checked={props.horizontal}
            onToggled={props.onHorizontalChange}
          />
        </Column>
      </Group>

      <Group title="Counter">
        <View direction={props.horizontal ? "row" : "column"} gap={8} padding={8}>
          <Text>{`current value: ${props.count}`}</Text>
          <Row gap={8}>
            <Button enabled={props.controlsEnabled} onClicked={() => props.onCountChange((value) => value - 1)}>
              -
            </Button>
            <Button enabled={props.controlsEnabled} onClicked={() => props.onCountChange(0)}>
              reset
            </Button>
            <Button enabled={props.controlsEnabled} onClicked={() => props.onCountChange((value) => value + 1)}>
              +
            </Button>
          </Row>
        </View>
      </Group>
    </Column>
  )
}
