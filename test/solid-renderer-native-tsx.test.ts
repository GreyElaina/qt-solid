import { describe, expect } from "vitest"

import {
  expectCleanExit,
  parseSnapshot,
  runBundledNodeScript,
  testIfNativeSupported,
} from "./mocking/native-run"

describe("native Solid renderer TSX", () => {
  testIfNativeSupported("createApp drives native snapshot through component TSX tree", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-solid-renderer-tsx-entry",
      entryExtension: ".tsx",
      entrySource: [
        "import { createSignal } from 'solid-js'",
        "import type { QtApp } from '@qt-solid/core'",
        "import { Button, Column, createApp, createWindow, Flex, Input, Label, Text } from '@qt-solid/solid'",
        "",
        "export async function run(app: QtApp) {",
        "  const [count, setCount] = createSignal(0)",
        "",
        "  const mainWindow = createWindow(",
        "    {",
        "      title: 'tsx-native',",
        "      width: 410,",
        "      height: 230,",
        "    },",
        "    () => (",
        "      <Flex direction='column' gap={7} padding={13}>",
        "        <Text>{'count: ' + count()}</Text>",
        "        <Label minWidth={90} minHeight={24} flexGrow={2} flexShrink={0}>status</Label>",
        "        <Button>+</Button>",
        "        <Input placeholder='name' minWidth={140} flexGrow={1} />",
        "      </Flex>",
        "    ),",
        "  )",
        "",
        "  const mounted = createApp(mainWindow).mount(app)",
        "  await Promise.resolve()",
        "  setCount(4)",
        "  await Promise.resolve()",
        "  await Promise.resolve()",
        "",
        "  console.log('SNAPSHOT', JSON.stringify(app.debugSnapshot()))",
        "  mounted.dispose()",
        "}",
      ].join("\n"),
    })

    expectCleanExit(result)

    const snapshot = parseSnapshot<{
      nodes: Array<{
        kind: string
        title?: string
        width?: number
        height?: number
        text?: string
        placeholder?: string
        flexDirection?: string
        gap?: number
        padding?: number
        minWidth?: number
        minHeight?: number
        flexGrow?: number
        flexShrink?: number
      }>
    }>(result.stdout)

    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "window" &&
          node.title === "tsx-native" &&
          node.width === 410 &&
          node.height === 230,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "view" &&
          node.flexDirection === "column" &&
          node.gap === 7 &&
          node.padding === 13,
      ),
    ).toBe(true)
    expect(snapshot.nodes.some((node) => node.kind === "text" && node.text === "count: 4")).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "label" &&
          node.text === "status" &&
          node.minWidth === 90 &&
          node.minHeight === 24 &&
          node.flexGrow === 2 &&
          node.flexShrink === 0,
      ),
    ).toBe(true)
    expect(snapshot.nodes.some((node) => node.kind === "button" && node.text === "+")).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "input" &&
          node.placeholder === "name" &&
          node.minWidth === 140 &&
          node.flexGrow === 1,
      ),
    ).toBe(true)
  })
})
