# calculator example

This example is now a workspace package:

- package: `@qt-solid-spike/example-calculator`
- path: `examples/calculator`

From repo root:

```bash
npm run example:calculator
npm run example:calculator:app
npm run example:calculator:devtools
```

From example workspace dir:

```bash
cd examples/calculator
npm run app
npm run devtools
```

Build-only / start-only:

```bash
cd examples/calculator
npm run build
npm run start
```

What it shows:

- `export default createApp(...)`
- composable `createWindow(...)` under app-level lifecycle hooks
- reactive window title tied to calculator display
- immediate-execution calculator state machine
- typed display editing through `Input`
- keypad layout built from `Row` / `Column` / `Group`
- button click wiring for digits, operators, clear, backspace, sign toggle, and evaluate

While running:

- `Ctrl+C` quits
- `npm run devtools` prints a CDP discovery endpoint for Chrome DevTools
