import { describe, expect } from "vitest"

import {
  expectCleanExit,
  nativeModuleSpecifier,
  runNodeScript,
  stripAnsi,
  testIfNativeSupported,
} from "./mocking/native-run"

describe("native Qt timer bridge", () => {
  testIfNativeSupported("debug timer events reach Node host", () => {
    const result = runNodeScript([
      `import { QtApp, __qtSolidDebugScheduleTimerEvent } from ${JSON.stringify(nativeModuleSpecifier)}`,
      "",
      "const app = QtApp.start((event) => {",
      "  console.log('EVENT', JSON.stringify(event))",
      "  if (event.type === 'debug' && event.name === 'qt-timer-bridge') {",
      "    app.shutdown()",
      "    process.exit(0)",
      "  }",
      "})",
      "__qtSolidDebugScheduleTimerEvent(25, 'qt-timer-bridge')",
      "setTimeout(() => {",
      "  console.error('TIMEOUT')",
      "  app.shutdown()",
      "  process.exit(2)",
      "}, 2000)",
    ].join("\n"))

    expectCleanExit(result)
    expect(stripAnsi(result.stdout)).toContain(
      'EVENT {"type":"debug","name":"qt-timer-bridge"}',
    )
  })
})
