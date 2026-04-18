/// <reference types="vite/client" />

import { acceptQtSolidDevAppHmr } from "@qt-solid/solid/hmr"

import { createCalculatorApp } from "./calculator-app.tsx"

const app = createCalculatorApp()

acceptQtSolidDevAppHmr(import.meta, app)

export default app

export { createCalculatorApp } from "./calculator-app.tsx"
