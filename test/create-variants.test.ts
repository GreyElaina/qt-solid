import { createComponent, createEffect, createRoot, createSignal, type Component } from "solid-js"
import { describe, expect, it } from "vitest"

import { createVariants } from "../packages/solid/src/app/motion/variants.ts"
import type { MotionComponentProps } from "../packages/solid/src/app/motion/types.ts"

describe("createVariants", () => {
  it("keeps resolved animate target reactive across variant changes", async () => {
    const seen: Array<Record<string, unknown> | undefined> = []

    const Base: Component<MotionComponentProps<object>> = (props) => {
      createEffect(() => {
        seen.push(props.animate as Record<string, unknown> | undefined)
      })
      return null as never
    }

    const Card = createVariants(Base, {
      base: { opacity: 1 },
      variants: {
        state: {
          idle: { y: 0 },
          lifted: { y: -12, scale: 1.06 },
        },
      },
      defaultVariants: { state: "idle" },
    })

    const root = createRoot((dispose) => {
      const [state, setState] = createSignal<"idle" | "lifted">("idle")
      createComponent(Card, {
        get state() {
          return state()
        },
      })
      return { dispose, setState }
    })

    await Promise.resolve()
    expect(seen.at(-1)).toMatchObject({ opacity: 1, y: 0 })

    root.setState("lifted")
    await Promise.resolve()
    expect(seen.at(-1)).toMatchObject({ opacity: 1, y: -12, scale: 1.06 })

    root.dispose()
  })
})
