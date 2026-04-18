import { describe, expect, it } from "vitest"

import { nodeArchiveInfo } from "../scripts/ensure-sea-node.mjs"

describe("sea node helper", () => {
  it("maps macOS archives deterministically", () => {
    expect(nodeArchiveInfo("25.8.1", "darwin", "arm64")).toEqual({
      directoryName: "node-v25.8.1-darwin-arm64",
      fileName: "node-v25.8.1-darwin-arm64.tar.gz",
      executableRelativePath: "node-v25.8.1-darwin-arm64/bin/node",
    })

    expect(nodeArchiveInfo("25.8.1", "darwin", "x64")).toEqual({
      directoryName: "node-v25.8.1-darwin-x64",
      fileName: "node-v25.8.1-darwin-x64.tar.gz",
      executableRelativePath: "node-v25.8.1-darwin-x64/bin/node",
    })
  })

  it("maps Linux and Windows archives deterministically", () => {
    expect(nodeArchiveInfo("25.8.1", "linux", "x64")).toEqual({
      directoryName: "node-v25.8.1-linux-x64",
      fileName: "node-v25.8.1-linux-x64.tar.xz",
      executableRelativePath: "node-v25.8.1-linux-x64/bin/node",
    })

    expect(nodeArchiveInfo("25.8.1", "linux", "arm64")).toEqual({
      directoryName: "node-v25.8.1-linux-arm64",
      fileName: "node-v25.8.1-linux-arm64.tar.xz",
      executableRelativePath: "node-v25.8.1-linux-arm64/bin/node",
    })

    expect(nodeArchiveInfo("25.8.1", "win32", "x64")).toEqual({
      directoryName: "node-v25.8.1-win-x64",
      fileName: "win-x64/node.exe",
      executableRelativePath: "node-v25.8.1-win-x64/node.exe",
    })
  })

  it("rejects unsupported targets", () => {
    expect(() => nodeArchiveInfo("25.8.1", "linux", "ppc64")).toThrow(/Unsupported Linux architecture/)
    expect(() => nodeArchiveInfo("25.8.1", "freebsd", "x64")).toThrow(/Unsupported platform/)
  })
})
