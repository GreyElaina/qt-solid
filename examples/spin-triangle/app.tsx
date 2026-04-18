/// <reference types="vite/client" />

import { acceptQtSolidDevAppHmr } from "@qt-solid/solid/hmr"

import { createSpinTriangleApp } from "./spin-triangle-app.tsx"

const app = createSpinTriangleApp()

acceptQtSolidDevAppHmr(import.meta, app)

export default app

export { createSpinTriangleApp } from "./spin-triangle-app.tsx"
