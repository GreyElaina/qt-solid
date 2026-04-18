import { describe, expect, it, vi } from "vitest"

import { createQtSolidAppSession } from "../packages/solid/src/entry/bootstrap"

function createHandle(label: string, calls: string[]) {
  const dispose = vi.fn(() => {
    calls.push(`${label}:dispose`)
  })

  const handle = {
    mount(_app: unknown, options?: { attachNativeEvents?: (handler: (event: unknown) => void) => void }) {
      options?.attachNativeEvents?.((event) => {
        calls.push(`${label}:event:${JSON.stringify(event)}`)
      })

      calls.push(`${label}:mount`)
      return { dispose }
    },
  }

  return {
    dispose,
    handle,
  }
}

describe("createQtSolidAppSession", () => {
  it("routes host events to the latest mounted app handle", () => {
    const calls: string[] = []
    const first = createHandle("first", calls)
    const second = createHandle("second", calls)

    const session = createQtSolidAppSession({ shutdown() {} } as never, first.handle)
    session.handleHostEvent({ type: "debug", name: "first" })
    session.replace(second.handle)
    session.handleHostEvent({ type: "debug", name: "second" })

    expect(calls).toEqual([
      "first:mount",
      'first:event:{"type":"debug","name":"first"}',
      "first:dispose",
      "second:mount",
      'second:event:{"type":"debug","name":"second"}',
    ])
  })

  it("rolls back to the previous app handle when replacement mount fails", () => {
    const calls: string[] = []
    const stable = createHandle("stable", calls)
    const session = createQtSolidAppSession({ shutdown() {} } as never, stable.handle)

    const failingHandle = {
      mount() {
        throw new Error("boom")
      },
    }

    expect(() => session.replace(failingHandle)).toThrow("boom")

    session.handleHostEvent({ type: "debug", name: "after-rollback" })

    expect(calls).toEqual([
      "stable:mount",
      "stable:dispose",
      "stable:mount",
      'stable:event:{"type":"debug","name":"after-rollback"}',
    ])
  })
})
