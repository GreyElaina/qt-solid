import { describe, expect } from "vitest";

import {
  expectCleanExit,
  parseSnapshot,
  runBundledNodeScript,
  stripAnsi,
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
          "import { canvasFragmentTreeSnapshot } from '@qt-solid/core/native'",
          "import { createApp, createWindow } from '@qt-solid/solid'",
          "",
          "export async function run(app: QtApp) {",
          "  const [offset, setOffset] = createSignal(0)",
          "",
          "  const mainWindow = createWindow(",
          "    {",
          "      title: 'motion-native',",
          "      width: 320,",
          "      height: 180,",
          "    },",
          "    () => (",
          "      <group gap={8}>",
          "        <rect",
          "          w={240}",
          "          h={80}",
          "          fill='#0078d4'",
          "          animate={{ x: offset(), opacity: offset() === 0 ? 1 : 0.5 }}",
          "          transition={{ duration: 0.02, ease: 'linear' }}",
          "        />",
          "      </group>",
          "    ),",
          "  )",
          "",
          "  const mounted = createApp(mainWindow).mount(app)",
          "  await Promise.resolve()",
          "  setOffset(24)",
          "  await new Promise((resolve) => setTimeout(resolve, 60))",
          "  const windowNode = app.debugSnapshot().nodes.find((node) => node.kind === 'window' && node.title === 'motion-native')",
          "  if (!windowNode) throw new Error('missing window')",
          "  console.log('SNAPSHOT', JSON.stringify({ app: app.debugSnapshot(), fragments: canvasFragmentTreeSnapshot(windowNode.id) }))",
          "  mounted.dispose()",
          "}",
        ].join("\n"),
      });

      expectCleanExit(result);

      const snapshot = parseSnapshot<{
        app: {
          nodes: Array<{
            kind: string;
            title?: string;
            width?: number;
            height?: number;
          }>;
        };
        fragments: Array<{ tag: string }>;
      }>(result.stdout);

      expect(
        snapshot.app.nodes.some(
          (node) =>
            node.kind === "window" &&
            node.title === "motion-native" &&
            node.width === 320 &&
            node.height === 180,
        ),
      ).toBe(true);
      expect(snapshot.fragments.some((node) => node.tag === "group")).toBe(true);
      expect(snapshot.fragments.some((node) => node.tag === "rect")).toBe(true);
    },
  );

  testIfNativeSupported(
    "inline intrinsic motion inside presence does not remount endlessly",
    async () => {
      const result = await runBundledNodeScript({
        tagPrefix: ".tmp-solid-renderer-inline-presence-motion-entry",
        entryExtension: ".tsx",
        entrySource: [
          "import { createSignal } from 'solid-js'",
          "import type { QtApp } from '@qt-solid/core'",
          "import { canvasFragmentSnapshotAnimations, canvasFragmentTreeSnapshot } from '@qt-solid/core/native'",
          "import { AnimatePresence, createApp, createWindow } from '@qt-solid/solid'",
          "",
          "function wait(ms: number) {",
          "  return new Promise((resolve) => setTimeout(resolve, ms))",
          "}",
          "",
          "export async function run(app: QtApp) {",
          "  const [show, setShow] = createSignal(true)",
          "  const mainWindow = createWindow(",
          "    { title: 'inline-presence-motion', width: 320, height: 200 },",
          "    () => (",
          "      <group gap={12}>",
          "        <AnimatePresence when={show()}>",
          "          {() => (",
          "            <rect",
          "              w={120} h={80} cornerRadius={12}",
          "              fill='#0078d4'",
          "              initial={{ opacity: 0, scale: 0.8, y: 20 }}",
          "              animate={{ opacity: 1, scale: 1, y: 0 }}",
          "              exit={{ opacity: 0, scale: 0.6, y: -20 }}",
          "              transition={{ type: 'spring', stiffness: 300, damping: 22 }}",
          "            />",
          "          )}",
          "        </AnimatePresence>",
          "      </group>",
          "    ),",
          "  )",
          "  const mounted = createApp(mainWindow).mount(app)",
          "  await wait(1000)",
          "  const windowNode = app.debugSnapshot().nodes.find((node) => node.kind === 'window' && node.title === 'inline-presence-motion')",
          "  if (!windowNode) throw new Error('missing window')",
          "  for (let i = 0; i < 4; i++) {",
          "    setShow(false)",
          "    await wait(250)",
          "    setShow(true)",
          "    await wait(250)",
          "  }",
          "  await wait(1000)",
          "  const fragments = canvasFragmentTreeSnapshot(windowNode.id)",
          "  const animations = canvasFragmentSnapshotAnimations(windowNode.id)",
          "  console.log('PROBE', JSON.stringify({ fragments: fragments.length, animations: animations.length }))",
          "  mounted.dispose()",
          "}",
        ].join("\n"),
      });

      expectCleanExit(result);

      const match = stripAnsi(result.stdout).match(/PROBE (\{.*\})/);
      expect(match).not.toBeNull();
      const probe = JSON.parse(match![1]!) as { fragments: number; animations: number };
      expect(probe.fragments).toBeLessThanOrEqual(3);
      expect(probe.animations).toBe(0);
    },
  );

  testIfNativeSupported(
    "staggered intrinsic motion preserves child delays on mount",
    async () => {
      const result = await runBundledNodeScript({
        tagPrefix: ".tmp-solid-renderer-stagger-motion-entry",
        entryExtension: ".tsx",
        entrySource: [
          "import type { QtApp } from '@qt-solid/core'",
          "import { canvasFragmentSnapshotAnimations, canvasFragmentTreeSnapshot } from '@qt-solid/core/native'",
          "import { createApp, createWindow } from '@qt-solid/solid'",
          "",
          "function wait(ms: number) {",
          "  return new Promise((resolve) => setTimeout(resolve, ms))",
          "}",
          "",
          "function delaySamples(canvasNodeId: number, rectIds: number[]) {",
          "  const animations = new Map(",
          "    canvasFragmentSnapshotAnimations(canvasNodeId).map((animation) => [animation.fragmentId, animation]),",
          "  )",
          "  return rectIds",
          "    .map((id) => ({",
          "      id,",
          "      delayMs: animations.get(id)?.channels.find((channel) => channel.property === 'x')?.delayMs ?? 0,",
          "    }))",
          "    .sort((a, b) => a.id - b.id)",
          "}",
          "",
          "export async function run(app: QtApp) {",
          "  const mainWindow = createWindow(",
          "    { title: 'stagger-motion', width: 320, height: 180 },",
          "    () => (",
          "      <group>",
          "        <group",
          "          row gap={8}",
          "          initial={{ opacity: 1 }}",
          "          animate={{ opacity: 1 }}",
          "          transition={{ delayChildren: 2, staggerChildren: 2 }}",
          "        >",
          "          <rect",
          "            w={24} h={24} fill='#ffffff'",
          "            initial={{ x: -16 }}",
          "            animate={{ x: 0 }}",
          "            transition={{ type: 'tween', duration: 0.4, ease: 'linear' }}",
          "          />",
          "          <rect",
          "            w={24} h={24} fill='#ffffff'",
          "            initial={{ x: -16 }}",
          "            animate={{ x: 0 }}",
          "            transition={{ type: 'tween', duration: 0.4, ease: 'linear' }}",
          "          />",
          "          <rect",
          "            w={24} h={24} fill='#ffffff'",
          "            initial={{ x: -16 }}",
          "            animate={{ x: 0 }}",
          "            transition={{ type: 'tween', duration: 0.4, ease: 'linear' }}",
          "          />",
          "        </group>",
          "      </group>",
          "    ),",
          "  )",
          "",
          "  const mounted = createApp(mainWindow).mount(app)",
          "  await wait(50)",
          "  const windowNode = app.debugSnapshot().nodes.find((node) => node.kind === 'window' && node.title === 'stagger-motion')",
          "  if (!windowNode) throw new Error('missing window')",
          "  const rectIds = canvasFragmentTreeSnapshot(windowNode.id)",
          "    .filter((node) => node.tag === 'rect' && node.width === 24 && node.height === 24)",
          "    .map((node) => node.id)",
          "    .sort((a, b) => a - b)",
          "  await wait(120)",
          "  const before = delaySamples(windowNode.id, rectIds)",
          "  await wait(2200)",
          "  const firstOnly = delaySamples(windowNode.id, rectIds)",
          "  console.log('STAGGER', JSON.stringify({ before, firstOnly }))",
          "  mounted.dispose()",
          "}",
        ].join("\n"),
      });

      expectCleanExit(result);

      const match = stripAnsi(result.stdout).match(/STAGGER (\{.*\})/);
      expect(match).not.toBeNull();
      const probe = JSON.parse(match![1]!) as {
        before: Array<{ id: number; delayMs: number }>;
        firstOnly: Array<{ id: number; delayMs: number }>;
      };

      expect(probe.before).toHaveLength(3);
      expect(probe.before[0]!.delayMs).toBeGreaterThan(500);
      expect(probe.before[1]!.delayMs).toBeGreaterThan(2500);
      expect(probe.before[2]!.delayMs).toBeGreaterThan(4500);
      expect(probe.firstOnly[0]!.delayMs).toBeLessThan(50);
      expect(probe.firstOnly[1]!.delayMs).toBeGreaterThan(300);
      expect(probe.firstOnly[2]!.delayMs).toBeGreaterThan(2000);
    },
  );
});
