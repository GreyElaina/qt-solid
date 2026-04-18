import type { AppHandle, WindowAllClosedContext, WindowHandle } from "@qt-solid/solid"
import { Column, Group, Text, View, createApp, createWindow } from "@qt-solid/solid"
import { SpinTriangle } from "@qt-solid/example-widgets"

export interface SpinTriangleAppOptions {
  onActivate?: () => void
  onWindowAllClosed?: (context: WindowAllClosedContext) => void
}

function createSpinTriangleWindow(): WindowHandle {
  return createWindow(
    {
      title: "spin_triangle",
      width: 420,
      height: 480,
    },
    () => (
      <Column gap={16} padding={20}>
        <Text>qt-solid · vello custom paint</Text>

        <Group title="spin_triangle">
          <View padding={16} alignItems="center" justifyContent="center">
            <SpinTriangle width={256} height={256} />
          </View>
        </Group>

        <Text>continuous rotation via requestNextFrame()</Text>
      </Column>
    ),
  )
}

export function createSpinTriangleApp(
  options: SpinTriangleAppOptions = {},
): AppHandle {
  return createApp(() => {
    const mainWindow = createSpinTriangleWindow()

    return {
      render: () => mainWindow.render(),
      onActivate() {
        mainWindow.open()
        options.onActivate?.()
      },
      onWindowAllClosed: options.onWindowAllClosed,
    }
  })
}
