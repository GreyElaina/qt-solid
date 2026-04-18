import { relative, resolve, sep } from "node:path"
import { fileURLToPath } from "node:url"

import { createServer, isRunnableDevEnvironment } from "vite"

import { createQtSolidVitePlugin } from "@qt-solid/solid/vite"

const entryArg = process.argv[2]
if (!entryArg) {
  throw new Error("missing dev entry argument")
}

process.env.NODE_ENV ??= "development"

const projectRoot = fileURLToPath(new URL("../", import.meta.url))
const entryPath = resolve(process.cwd(), entryArg)
const entryModuleId = `/${relative(projectRoot, entryPath).split(sep).join("/")}`

const server = await createServer({
  appType: "custom",
  configFile: false,
  environments: {
    ssr: {
      consumer: "server",
      dev: {
        moduleRunnerTransform: true,
      },
      resolve: {
        conditions: ["browser", "development"],
      },
    },
  },
  plugins: [createQtSolidVitePlugin()],
  root: projectRoot,
  server: {
    middlewareMode: true,
  },
})

const environment = server.environments.ssr
if (!isRunnableDevEnvironment(environment)) {
  throw new Error("expected Vite SSR environment to provide a module runner")
}

const globalObject = globalThis
globalObject.__qtSolidViteDevServer__ = server
globalObject.__qtSolidViteDevEnvironment__ = environment

let closing = false

const close = async () => {
  if (closing) {
    return
  }

  closing = true
  await server.close()
}

process.once("SIGINT", () => {
  void close()
})

process.once("SIGTERM", () => {
  void close()
})

try {
  await environment.runner.import(entryModuleId)
} catch (error) {
  await close()
  throw error
}
