import { resolve } from "node:path"
import { pathToFileURL } from "node:url"

const entry = process.argv[2]
if (!entry) {
  throw new Error("missing entry file argument")
}

process.env.QT_SOLID_DEVTOOLS ??= "1"

await import(pathToFileURL(resolve(process.cwd(), entry)).href)
