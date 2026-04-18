# qt-solid-spike

Qt + Solid custom renderer spike for stock Node.js.

## Install

```bash
npm install
```

macOS native builds expect `llvm-ar` on `PATH`.

## TSX setup

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "jsx": "preserve",
    "jsxImportSource": "@qt-solid/solid"
  }
}
```

## Build with rolldown

Primary path now uses `rolldown` and emits self-booting Node bundles.

`rolldown.config.mjs`:

```js
import { defineConfig } from "rolldown"
import qtSolidRolldownPlugin from "@qt-solid/solid/rolldown"

export default defineConfig({
  input: "./src/app.tsx",
  output: {
    file: "./dist/app.js",
    format: "esm",
    sourcemap: true,
  },
  platform: "node",
  plugins: [qtSolidRolldownPlugin],
  resolve: {
    conditionNames: ["browser"],
  },
})
```

`package.json`:

```json
{
  "scripts": {
    "build": "rolldown -c",
    "start": "node --enable-source-maps --conditions=browser ./dist/app.js"
  }
}
```

Build and run:

```bash
npm run build
npm run start
```

## Vite compatibility

`vite-plugin` stays available for existing Vite setups.

`vite.config.ts`:

```ts
import { defineConfig } from "vite"
import qtSolidVitePlugin from "@qt-solid/solid/vite"

export default defineConfig({
  plugins: [qtSolidVitePlugin],
})
```

## High-level app model

High-level ergonomic path stays primary. Top-level windowing is modeled as composable handle under `createApp`, not raw JSX element.

```tsx
import { Column, Label, Text, createApp, createWindow } from "@qt-solid/solid"

export default createApp(() => {
  const mainWindow = createWindow(
    {
      title: "demo",
      width: 420,
      height: 240,
    },
    () => (
      <Column gap={8} padding={12}>
        <Text>hello</Text>
        <Label minWidth={80}>status</Label>
      </Column>
    ),
  )

  return {
    render: () => mainWindow.render(),
    onWindowAllClosed: ({ quit }) => quit(),
    onActivate: () => mainWindow.open(),
  }
})
```

`createApp(...)` now owns app-level lifecycle policy.
If a window has no explicit `onCloseRequested`, closing it hides that window; when last open window disappears, `onWindowAllClosed` runs.
`onActivate` can reopen hidden windows.
Native runtime now enables `window-host` by default on supported platforms;
legacy non-`window-host` fallback is gone.
Low-level primitive `renderQt(...)` still exists for embedding/tests.
Low-level intrinsic props still exist for ordinary widgets;
`Row` / `Column` remain sugar over `View`.

## Validate

```bash
npm run typecheck
npm run test
```

## Native demo

```bash
npm run debug:demo
```

## Counter example

`examples/counter` is now a workspace package.

From repo root:

```bash
npm run example:counter
npm run example:counter:app
npm run example:counter:devtools
```

From workspace dir:

```bash
cd examples/counter
npm run app
npm run devtools
```

`npm run run` is kept as shorthand for `npm run app`.

## Calculator example

`examples/calculator` is now a workspace package.

From repo root:

```bash
npm run example:calculator
npm run example:calculator:app
npm run example:calculator:devtools
```

From workspace dir:

```bash
cd examples/calculator
npm run app
npm run devtools
```

## DevTools / inspector

Set `QT_SOLID_DEVTOOLS=1` or use one of the `*:devtools` scripts.
Sourcemaps are now emitted by the default rolldown path, so Chrome DevTools Sources can map bundled code back to TS/TSX when examples run with `--enable-source-maps`.
Bootstrap logs a discovery endpoint like:

```text
[qt-solid devtools] http://127.0.0.1:9229/json/list
```

Those targets expose two debug surfaces:

- `qt-solid-renderer`: renderer tree (`window` / `view` / `group` / `button` / `input` / `text`) with forwarded Sources / Debugger domains
- `qt-solid-components`: mapped component tree (`Window` / `Column` / `Group` / `Input` ...) with the same forwarded Sources / Debugger domains

Use renderer target when you care about host structure and native highlight. Use components target when you care about Solid ownership/component callsites.
Renderer nodes still keep source/owner/reveal metadata, but the inline Elements tree is intentionally quieter now; richer debug data lives in Stack Trace and Runtime properties instead of being printed into every row.
Synthetic DOM nodes answer `DOM.getNodeStackTraces`, so Elements Stack Trace can reveal owner/source frames into Sources using the same metadata. Source reveal now uses canonical compile-time `file://` locations instead of guessing from runtime `cwd`, and Runtime synthetic node objects also expose direct `sourceLocation` / `creationLocation` / `creationFrames` payloads for easier reveal-oriented inspection.
Breakpoint note: renderer/components targets now run through a worker-thread broker. Synthetic DOM / component data is served from a mirrored snapshot in that worker, so pausing JS no longer forces Elements-side synthetic RPC to wait on main-thread DOM state. Native-only geometry / inspect helpers (`DOM.getBoxModel`, hit-test, highlight) stay best-effort while paused and may fall back to cached or empty results until execution resumes.
Component target nodes expose component-oriented Runtime properties such as `componentName`, `componentPath`, `rendererNodeId`, `rendererKind`, `sourceLocation`, and `frameKind`.
`DOM.getBoxModel` now uses native node bounds, `DOM.getNodeForLocation` uses native hit-testing, and `Overlay.highlightNode` lowers to a native debug primitive, so selected Elements nodes trigger a real Qt-side inspector box instead of JS-only fake overlay.
`Overlay.setInspectMode` now enables native inspect mode, and native hit results flow back through `Overlay.inspectNodeRequested` for both renderer and components targets.
