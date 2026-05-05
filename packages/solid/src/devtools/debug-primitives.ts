import type { QtApp } from "@qt-solid/core/native"
import {
  canvasFragmentGetWorldBounds,
  canvasFragmentHitTest,
  canvasFragmentSetDebugHighlight,
  canvasFragmentSnapshotAnimations,
  canvasFragmentSnapshotLayers,
  canvasFragmentTreeSnapshot,
  captureCanvasRegion,
  captureCanvasSnapshot,
  captureFragmentIsolated,
} from "@qt-solid/core/native"

export function createDebugPrimitives(app: QtApp) {
  return {
    highlightNode(nodeId: number): void {
      app.getNode(nodeId).highlight()
    },
    getNodeBounds(nodeId: number) {
      return app.getNode(nodeId).getBounds()
    },
    getNodeAtPoint(screenX: number, screenY: number) {
      return app.getNodeAtPoint(screenX, screenY)
    },
    setInspectMode(enabled: boolean): void {
      app.setInspectMode(enabled)
    },
    clearHighlight(): void {
      app.clearHighlight()
    },
    highlightFragment(canvasNodeId: number, fragmentId: number | null): void {
      canvasFragmentSetDebugHighlight(canvasNodeId, fragmentId ?? undefined)
    },
    getFragmentBounds(canvasNodeId: number, fragmentId: number) {
      const bounds = canvasFragmentGetWorldBounds(canvasNodeId, fragmentId)
      if (!bounds) {
        return { visible: false, screenX: 0, screenY: 0, width: 0, height: 0 }
      }
      return { visible: true, screenX: bounds.x, screenY: bounds.y, width: bounds.width, height: bounds.height }
    },
    fragmentHitTest(canvasNodeId: number, x: number, y: number) {
      return canvasFragmentHitTest(canvasNodeId, x, y)
    },
    fragmentTreeSnapshot(canvasNodeId: number) {
      return canvasFragmentTreeSnapshot(canvasNodeId)
    },
    snapshotLayers(canvasNodeId: number) {
      return canvasFragmentSnapshotLayers(canvasNodeId)
    },
    snapshotAnimations(canvasNodeId: number) {
      return canvasFragmentSnapshotAnimations(canvasNodeId)
    },
    captureFragmentRegion(canvasNodeId: number, x: number, y: number, width: number, height: number): string | null {
      const pngBytes = captureCanvasRegion(canvasNodeId, x, y, width, height)
      if (!pngBytes || pngBytes.length === 0) {
        return null
      }
      return `data:image/png;base64,${Buffer.from(pngBytes).toString("base64")}`
    },
    captureFragmentIsolated(canvasNodeId: number, fragmentId: number): string | null {
      const pngBytes = captureFragmentIsolated(canvasNodeId, fragmentId)
      if (!pngBytes || pngBytes.length === 0) {
        return null
      }
      return `data:image/png;base64,${Buffer.from(pngBytes).toString("base64")}`
    },
    captureAllFragmentsIsolated(canvasNodeId: number, fragmentIds: number[]): Record<number, string> {
      const result: Record<number, string> = {}
      for (const fid of fragmentIds) {
        const pngBytes = captureFragmentIsolated(canvasNodeId, fid)
        if (pngBytes && pngBytes.length > 0) {
          result[fid] = `data:image/png;base64,${Buffer.from(pngBytes).toString("base64")}`
        }
      }
      return result
    },
    captureCanvasFullSnapshot(canvasNodeId: number) {
      return captureCanvasSnapshot(canvasNodeId)
    },
  }
}

export type QtSolidDebugPrimitives = ReturnType<typeof createDebugPrimitives>
