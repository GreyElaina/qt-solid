# counter example

This example is now a workspace package:

- package: `@qt-solid-spike/example-counter`
- path: `examples/counter`

From repo root:

```bash
npm run example:counter
npm run example:counter:app
npm run example:counter:devtools
```

From example workspace dir:

```bash
cd examples/counter
npm run app
npm run devtools
```

Build-only / start-only:

```bash
cd examples/counter
npm run build
npm run start
```

`npm run run` is kept as shorthand for `npm run app`.

What it shows:

- `export default createApp(...)`
- `createApp(() => ({ render, onWindowAllClosed, onActivate }))`
- reactive window props through composable window handles
- `Column` / `Row` sugar
- `View` as layout carrier
- button click wiring
- input text change wiring
- checkbox toggle wiring
- reactive window title + reactive text update

While running:

- `Ctrl+C` quits
- `npm run devtools` prints a CDP discovery endpoint for Chrome DevTools
