import { describe, expect, it } from "vitest"

import { __testMotionInternals } from "../packages/solid/src/app/motion.ts"

describe("motion transition lowering", () => {
  it("keeps default spring when transition only specifies per-property overrides", () => {
    const lowered = __testMotionInternals.lowerTransition({
      x: {
        type: "tween",
        duration: 0.25,
      },
    })

    expect(lowered.default).toEqual({ type: "spring" })
    expect(lowered.x).toEqual({
      type: "tween",
      duration: 0.25,
      ease: undefined,
    })
  })

  it("preserves cubic bezier arrays", () => {
    const lowered = __testMotionInternals.lowerTransition({
      type: "tween",
      duration: 0.4,
      ease: [0.68, -0.6, 0.32, 1.6],
    })

    expect(lowered.default).toEqual({
      type: "tween",
      duration: 0.4,
      ease: [0.68, -0.6, 0.32, 1.6],
    })
  })
})
