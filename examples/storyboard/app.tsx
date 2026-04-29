/// <reference types="vite/client" />

import { acceptQtSolidDevAppHmr } from "@qt-solid/solid/hmr"

import { createStoryboardApp } from "./storyboard-app.tsx"

const app = createStoryboardApp()

acceptQtSolidDevAppHmr(import.meta, app)

export default app

export { createStoryboardApp } from "./storyboard-app.tsx"
