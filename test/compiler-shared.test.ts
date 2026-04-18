import { pathToFileURL } from "node:url"

import { describe, expect, it } from "vitest"

import { transformQtSolidModule } from "../packages/solid/src/build/compiler-shared.js"

describe("transformQtSolidModule", () => {
  it("injects source metadata into JSX and createWindow call sites", async () => {
    const transformed = await transformQtSolidModule(
      [
        'import { Text, createWindow } from "@qt-solid/solid"',
        '',
        'export function makeWindow() {',
        '  return createWindow({ title: "demo" }, () => <Text>Hello</Text>)',
        '}',
      ].join("\n"),
      {
        filename: `${process.cwd()}/examples/calculator/app.tsx`,
        moduleName: "@qt-solid/solid",
        sourceMaps: true,
      },
    )

    expect(transformed?.code).toContain("__qtSolidSource")
    expect(transformed?.code).toContain('fileName: "examples/calculator/app.tsx"')
    expect(transformed?.code).toContain(`fileUrl: "${pathToFileURL(`${process.cwd()}/examples/calculator/app.tsx`).href}"`)
    expect(transformed?.code).toContain(`projectRootUrl: "${pathToFileURL(`${process.cwd()}/`).href}"`)
    expect(transformed?.code).toContain('from "@qt-solid/solid/compiler-rt"')
    expect(transformed?.code).toContain("withQtSourceMeta as")
    expect(transformed?.code).toMatch(/createWindow\([^\n]*qtSolidWithSourceMeta/i)
    expect(transformed?.map).not.toBeNull()
    expect((transformed?.map as { sources?: string[] } | null)?.sources).toContain("examples/calculator/app.tsx")
  })

  it("injects solid refresh hooks for Vite HMR in dev mode", async () => {
    const transformed = await transformQtSolidModule(
      [
        'import { Text } from "@qt-solid/solid"',
        "",
        "export function CounterView() {",
        "  return <Text>hot</Text>",
        "}",
      ].join("\n"),
      {
        filename: `${process.cwd()}/examples/counter/counter-view.tsx`,
        hmr: {
          bundler: "vite",
          enabled: true,
        },
        moduleName: "@qt-solid/solid",
        sourceMaps: true,
      },
    )

    expect(transformed?.code).toContain("solid-refresh")
    expect(transformed?.code).toContain("import.meta.hot.accept()")
    expect(transformed?.code).toContain("CounterView")
    expect(transformed?.code).toContain('from "@qt-solid/solid/compiler-rt"')
  })
})
