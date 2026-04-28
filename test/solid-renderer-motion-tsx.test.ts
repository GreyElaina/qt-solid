import { describe, expect } from "vitest";

import {
  expectCleanExit,
  parseSnapshot,
  runBundledNodeScript,
  testIfNativeSupported,
} from "./mocking/native-run";

describe("native Solid renderer motion TSX", () => {
  testIfNativeSupported(
    "motion components mount and update without repaint-only fallback",
    async () => {
      const result = await runBundledNodeScript({
        tagPrefix: ".tmp-solid-renderer-motion-entry",
        entryExtension: ".tsx",
        entrySource: [
          "import { createSignal } from 'solid-js'",
          "import type { QtApp } from '@qt-solid/core'",
          "import { createApp, createWindow, motion, Text, View } from '@qt-solid/solid'",
          "",
          "export async function run(app: QtApp) {",
          "  const [offset, setOffset] = createSignal(0)",
          "  const Card = (props: { width: number; height: number; title: string }) => (",
          "    <View width={props.width} height={props.height}>",
          "      <Text>{props.title}</Text>",
          "    </View>",
          "  )",
          "  const MotionCard = motion(Card)",
          "",
          "  const mainWindow = createWindow(",
          "    {",
          "      title: 'motion-native',",
          "      width: 320,",
          "      height: 180,",
          "    },",
          "    () => (",
          "      <MotionCard",
          "        title='motion body'",
          "        width={240}",
          "        height={80}",
          "        animate={{ x: offset(), opacity: offset() === 0 ? 1 : 0.5 }}",
          "        transition={{ duration: 0.02, ease: 'linear' }}",
          "      />",
          "    ),",
          "  )",
          "",
          "  const mounted = createApp(mainWindow).mount(app)",
          "  await Promise.resolve()",
          "  setOffset(24)",
          "  await new Promise((resolve) => setTimeout(resolve, 60))",
          "  console.log('SNAPSHOT', JSON.stringify(app.debugSnapshot()))",
          "  mounted.dispose()",
          "}",
        ].join("\n"),
      });

      expectCleanExit(result);

      const snapshot = parseSnapshot<{
        nodes: Array<{
          kind: string;
          title?: string;
          width?: number;
          height?: number;
          text?: string;
        }>;
      }>(result.stdout);

      expect(
        snapshot.nodes.some(
          (node) =>
            node.kind === "window" &&
            node.title === "motion-native" &&
            node.width === 320 &&
            node.height === 180,
        ),
      ).toBe(true);
      expect(snapshot.nodes.some((node) => node.kind === "view")).toBe(true);
      expect(
        snapshot.nodes.some(
          (node) => node.kind === "text" && node.text === "motion body",
        ),
      ).toBe(true);
    },
  );
});
