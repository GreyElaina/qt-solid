/// <reference types="vite/client" />

import { acceptQtSolidDevAppHmr } from "@qt-solid/solid/hmr"

import { createCounterApp } from "./counter-app.tsx"

const app = createCounterApp()

acceptQtSolidDevAppHmr(import.meta, app)

export default app

export { createCounterApp } from "./counter-app.tsx"
