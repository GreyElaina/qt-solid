import { Worker } from "node:worker_threads"

const port = Number(process.env.QT_SOLID_DEVTOOLS_PORT ?? "9460")

const snapshot = {
  rootId: 1,
  revision: 1,
  nodes: [
    {
      id: 1,
      kind: "qt-root",
      text: null,
      source: null,
      owner: null,
      props: {},
      listeners: [],
      parentId: null,
      childIds: [2],
    },
    {
      id: 2,
      kind: "window",
      text: null,
      source: {
        fileName: "examples/devtools-demo.tsx",
        lineNumber: 12,
        columnNumber: 7,
        fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
        projectRootUrl: new URL("../", import.meta.url).href,
      },
      owner: {
        ownerStack: [
          {
            componentName: "DevtoolsDemo",
            source: {
              fileName: "examples/devtools-demo.tsx",
              lineNumber: 11,
              columnNumber: 3,
              fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
              projectRootUrl: new URL("../", import.meta.url).href,
            },
          },
          {
            componentName: "Window",
            source: {
              fileName: "examples/devtools-demo.tsx",
              lineNumber: 12,
              columnNumber: 7,
              fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
              projectRootUrl: new URL("../", import.meta.url).href,
            },
          },
        ],
      },
      props: {
        title: "debugger-child",
      },
      listeners: [],
      parentId: 1,
      childIds: [3],
    },
    {
      id: 3,
      kind: "input",
      text: null,
      source: {
        fileName: "examples/devtools-demo.tsx",
        lineNumber: 15,
        columnNumber: 9,
        fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
        projectRootUrl: new URL("../", import.meta.url).href,
      },
      owner: {
        ownerStack: [
          {
            componentName: "DevtoolsDemo",
            source: {
              fileName: "examples/devtools-demo.tsx",
              lineNumber: 11,
              columnNumber: 3,
              fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
              projectRootUrl: new URL("../", import.meta.url).href,
            },
          },
          {
            componentName: "Window",
            source: {
              fileName: "examples/devtools-demo.tsx",
              lineNumber: 12,
              columnNumber: 7,
              fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
              projectRootUrl: new URL("../", import.meta.url).href,
            },
          },
          {
            componentName: "Input",
            source: {
              fileName: "examples/devtools-demo.tsx",
              lineNumber: 15,
              columnNumber: 9,
              fileUrl: new URL("../examples/devtools-demo.tsx", import.meta.url).href,
              projectRootUrl: new URL("../", import.meta.url).href,
            },
          },
        ],
      },
      props: {
        text: "hello",
        placeholder: "0",
      },
      listeners: [],
      parentId: 2,
      childIds: [],
    },
  ],
}

const worker = new Worker(new URL("../../packages/solid/src/devtools/cdp-worker.mjs", import.meta.url), {
  workerData: { port },
})

worker.on("message", (message) => {
  if (message?.type === "ready") {
    worker.postMessage({
      type: "mirror-update",
      snapshot,
      mutation: { type: "document-reset" },
    })
    process.stdout.write(`${JSON.stringify({ type: "ready", url: message.url })}\n`)
    return
  }

  if (message?.type !== "native-request") {
    return
  }

  const requestId = message.requestId
  switch (message.method) {
    case "highlightNode":
    case "setInspectMode":
    case "clearHighlight": {
      worker.postMessage({ type: "native-response", requestId, result: {} })
      return
    }
    case "getNodeBounds": {
      worker.postMessage({
        type: "native-response",
        requestId,
        result: {
          visible: true,
          screenX: 40,
          screenY: 60,
          width: 120,
          height: 32,
        },
      })
      return
    }
    case "getNodeAtPoint": {
      worker.postMessage({
        type: "native-response",
        requestId,
        result: 3,
      })
      return
    }
    default: {
      worker.postMessage({
        type: "native-response",
        requestId,
        error: `Unsupported native method ${message.method}`,
      })
    }
  }
})

worker.on("error", (error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`)
  process.exitCode = 1
})

const shutdown = async () => {
  await worker.terminate()
}

process.on("SIGTERM", () => {
  void shutdown().finally(() => process.exit(0))
})

process.on("SIGINT", () => {
  void shutdown().finally(() => process.exit(0))
})

setInterval(() => {}, 1_000)
