import { createComponent, createRoot, createSignal } from "solid-js"
import { describe, expect, it } from "vitest"

import { AnimatePresence } from "../packages/solid/src/app/motion/presence.ts"

describe("AnimatePresence", () => {
  it("mounts children when `when` flips from false to true", async () => {
    let mounts = 0

    const root = createRoot((dispose) => {
      const [show, setShow] = createSignal(false)

      createComponent(AnimatePresence, {
        get when() {
          return show()
        },
        children: () => {
          mounts += 1
          return null
        },
      })

      return { dispose, setShow }
    })

    await Promise.resolve()
    expect(mounts).toBe(0)

    root.setShow(true)
    await Promise.resolve()
    expect(mounts).toBe(1)

    root.dispose()
  })
})
