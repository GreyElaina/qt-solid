#!/usr/bin/env node

import { resolve } from "node:path"
import { createInterface } from "node:readline"
import { resolvePreviewableExports } from "./resolve-exports.ts"
import { createPreviewServer } from "./preview-server.ts"
import { loadConfig } from "./load-config.ts"
import type { PreviewPropsSchema } from "./protocol.ts"

const PREFIX = "[qt-solid preview]"

function usage(): never {
  console.error(`Usage: qt-solid-preview <file> [exportName]`)
  console.error(`\nReads qt-solid.config.ts for theme, components, and preview settings.`)
  process.exit(1)
}

function printSchema(schema: PreviewPropsSchema): void {
  console.log(`\n${PREFIX} Component: ${schema.componentName}`)
  if (schema.props.length > 0) {
    console.log(`${PREFIX} Props:`)
    for (const p of schema.props) {
      const extra = p.values ? ` [${p.values.join(" | ")}]` : ""
      const def = p.defaultValue !== undefined ? ` = ${JSON.stringify(p.defaultValue)}` : ""
      console.log(`  ${p.name}: ${p.type}${extra}${def}`)
    }
  }
  if (schema.variantAxes.length > 0) {
    console.log(`${PREFIX} Variant axes:`)
    for (const a of schema.variantAxes) {
      const def = a.defaultValue ? ` (default: ${a.defaultValue})` : ""
      console.log(`  ${a.name}: ${a.values.join(" | ")}${def}`)
    }
  }
}

function startInteractive(server: ReturnType<typeof createPreviewServer> extends Promise<infer T> ? T : never): void {
  const rl = createInterface({ input: process.stdin, output: process.stdout })

  console.log(`\n${PREFIX} Interactive mode. Commands:`)
  console.log(`  set <prop> <value>    — set a prop value`)
  console.log(`  variant <axis> <val>  — set variant axis`)
  console.log(`  reset                 — reset all props`)
  console.log(`  schema                — print current schema`)
  console.log(`  q                     — quit\n`)

  rl.on("line", (line) => {
    const parts = line.trim().split(/\s+/)
    const cmd = parts[0]

    switch (cmd) {
      case "set": {
        const name = parts[1]
        const raw = parts.slice(2).join(" ")
        if (!name) { console.log("  usage: set <prop> <value>"); break }
        let value: unknown = raw
        if (raw === "true") value = true
        else if (raw === "false") value = false
        else if (raw !== "" && !isNaN(Number(raw))) value = Number(raw)
        server.send({ type: "set-prop", name, value })
        console.log(`  ${name} = ${JSON.stringify(value)}`)
        break
      }
      case "variant": {
        const axis = parts[1]
        const val = parts[2]
        if (!axis || !val) { console.log("  usage: variant <axis> <value>"); break }
        server.send({ type: "set-variant", axis, value: val })
        console.log(`  ${axis} = ${val}`)
        break
      }
      case "reset":
        server.send({ type: "reset-props" })
        console.log("  props reset")
        break
      case "schema":
        if (server.schema) printSchema(server.schema)
        else console.log("  no schema yet")
        break
      case "q":
      case "quit":
      case "exit":
        rl.close()
        server.dispose()
        process.exit(0)
        break
      default:
        if (cmd) console.log(`  unknown command: ${cmd}`)
    }
  })

  rl.on("close", () => {
    server.dispose()
    process.exit(0)
  })
}

async function main() {
  const config = await loadConfig()

  const [file, exportName] = process.argv.slice(2)
  if (!file) usage()

  const filePath = resolve(file)
  let target: string

  if (exportName) {
    target = exportName
  } else {
    const exports = await resolvePreviewableExports(filePath)
    if (exports.length === 0) {
      console.error(`${PREFIX} No previewable exports found in ${file}`)
      process.exit(1)
    }
    const first = exports[0]!
    target = first.kind === "default" ? "default" : first.name
    if (exports.length > 1) {
      console.log(`${PREFIX} Found exports: ${exports.map(e => e.name).join(", ")}`)
      console.log(`${PREFIX} Using first: ${target}`)
    }
  }

  console.log(`${PREFIX} Previewing: ${target} from ${file}`)

  const server = await createPreviewServer({
    filePath,
    exportName: target,
    themeFile: config.theme,
    wsPort: config.preview?.wsPort,
    width: config.preview?.width,
    height: config.preview?.height,
  })

  server.onMessage((msg) => {
    if (msg.type === "schema") {
      printSchema(msg.schema)
    } else if (msg.type === "ready") {
      console.log(`${PREFIX} Preview ready`)
    } else if (msg.type === "error") {
      console.error(`${PREFIX} Error: ${msg.message}`)
    }
  })

  console.log(`${PREFIX} Connect Chrome DevTools to inspect`)

  const dispose = () => {
    server.dispose()
    process.exit(0)
  }

  process.on("SIGINT", dispose)
  process.on("SIGTERM", dispose)

  if (process.stdin.isTTY) {
    startInteractive(server)
  }
}

main().catch((err) => {
  console.error(`${PREFIX}`, err)
  process.exit(1)
})
