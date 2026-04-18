import {
  __qtSolidDebugClearHighlight,
  __qtSolidDebugGetNodeAtPoint,
  __qtSolidDebugGetNodeBounds,
  __qtSolidDebugHighlightNode,
  __qtSolidDebugSetInspectMode,
} from "@qt-solid/core"

export const qtSolidDebugPrimitives = {
  highlightNode(nodeId: number): void {
    __qtSolidDebugHighlightNode(nodeId)
  },
  getNodeBounds(nodeId: number) {
    return __qtSolidDebugGetNodeBounds(nodeId)
  },
  getNodeAtPoint(screenX: number, screenY: number) {
    return __qtSolidDebugGetNodeAtPoint(screenX, screenY)
  },
  setInspectMode(enabled: boolean): void {
    __qtSolidDebugSetInspectMode(enabled)
  },
  clearHighlight(): void {
    __qtSolidDebugClearHighlight()
  },
}
