import { describe, expect } from "vitest"

import {
  expectCleanExit,
  parseSnapshot,
  runBundledNodeScript,
  testIfNativeSupported,
} from "./mocking/native-run"

describe("native Solid renderer intrinsic TSX", () => {
  testIfNativeSupported("createApp drives native snapshot through intrinsic widget TSX tree", async () => {
    const result = await runBundledNodeScript({
      tagPrefix: ".tmp-solid-renderer-intrinsic-tsx-entry",
      entryExtension: ".tsx",
      entrySource: [
        "import type { QtApp } from '@qt-solid/core'",
        "import { createApp, createWindow } from '@qt-solid/solid'",
        "",
        "export async function run(app: QtApp) {",
        "  const mainWindow = createWindow(",
        "    {",
        "      title: 'intrinsic-native',",
        "      width: 300,",
        "      height: 160,",
        "    },",
        "    () => (",
        "      <view direction='column' gap={5} padding={9}>",
        "        <text text='hello' />",
        "        <label text='status' minWidth={90} minHeight={24} grow={2} shrink={0} />",
        "        <button text='+' />",
        "        <input placeholder='name' text='Akashina' minWidth={140} grow={1} />",
        "      </view>",
        "    ),",
        "  )",
        "",
        "  const mounted = createApp(mainWindow).mount(app)",
        "  await Promise.resolve()",
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
          node.title === "intrinsic-native" &&
          node.width === 300 &&
          node.height === 160,
      ),
    ).toBe(true)
    expect(
      snapshot.nodes.some(
        (node) =>
          node.kind === "view" &&
          node.flexDirection === "column" &&
          node.gap === 5 &&
          node.padding === 9,
      ),
    ).toBe(true)
    expect(snapshot.nodes.some((node) => node.kind === "text" && node.text === "hello")).toBe(true)
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
          node.text === "Akashina" &&
          node.minWidth === 140 &&
          node.flexGrow === 1,
      ),
    ).toBe(true)
  })
})
