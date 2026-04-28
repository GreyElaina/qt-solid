/// <reference types="vite/client" />

import { acceptQtSolidDevAppHmr } from "@qt-solid/solid/hmr"

import { createRenderingGalleryApp } from "./rendering-gallery-app.tsx"

const app = createRenderingGalleryApp()

acceptQtSolidDevAppHmr(import.meta, app)

export default app

export { createRenderingGalleryApp } from "./rendering-gallery-app.tsx"
