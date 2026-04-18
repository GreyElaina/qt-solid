import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { pathToFileURL } from "node:url"

import { describe, expect, it } from "vitest"

import { qtSolidSourceMetadataUrl, serializeQtSolidSourceLocation } from "../packages/solid/src/devtools/source-metadata"

describe("qt solid source metadata", () => {
  it("prefers canonical fileUrl over runtime cwd fallback", () => {
    const originalCwd = process.cwd()
    const tempCwd = mkdtempSync(join(tmpdir(), "qt-solid-source-meta-"))
    const sourceUrl = pathToFileURL(resolve("examples/counter/app.tsx")).href

    try {
      process.chdir(tempCwd)
      expect(
        qtSolidSourceMetadataUrl({
          fileName: "app.tsx",
          lineNumber: 1,
          columnNumber: 1,
          fileUrl: sourceUrl,
        }),
      ).toBe(sourceUrl)
    } finally {
      process.chdir(originalCwd)
      rmSync(tempCwd, { recursive: true, force: true })
    }
  })

  it("classifies project-local callsites as user and node_modules callsites as library", () => {
    const projectRootUrl = pathToFileURL(`${process.cwd()}/`).href

    expect(
      serializeQtSolidSourceLocation({
        fileName: "examples/counter/app.tsx",
        lineNumber: 1,
        columnNumber: 1,
        fileUrl: pathToFileURL(resolve("examples/counter/app.tsx")).href,
        projectRootUrl,
      }),
    ).toMatchObject({
      frameKind: "user",
    })

    expect(
      serializeQtSolidSourceLocation({
        fileName: "node_modules/example/index.tsx",
        lineNumber: 1,
        columnNumber: 1,
        fileUrl: pathToFileURL(resolve("node_modules/example/index.tsx")).href,
        projectRootUrl,
      }),
    ).toMatchObject({
      frameKind: "library",
    })
  })
})
