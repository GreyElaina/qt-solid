// ---------------------------------------------------------------------------
// qt-solid Figma Plugin — main thread (sandbox)
//
// Bidirectional bridge: Design ↔ Code
// - Watches selection/document changes → forwards to UI
// - Receives commands from UI → modifies canvas or reads data
// - Tags nodes with qt-solid component metadata via pluginData
// - Syncs design tokens via Variables API
// ---------------------------------------------------------------------------

const QT_SOLID_NS = "qt-solid"
const KEY_COMPONENT = "componentExport"
const KEY_FILE = "componentFile"

// ---------------------------------------------------------------------------
// Plugin init
// ---------------------------------------------------------------------------

figma.showUI(__html__, { width: 360, height: 480 })

// ---------------------------------------------------------------------------
// Selection → UI
// ---------------------------------------------------------------------------

function serializeNode(node: SceneNode): Record<string, unknown> {
  const data: Record<string, unknown> = {
    id: node.id,
    name: node.name,
    type: node.type,
    x: Math.round(node.x),
    y: Math.round(node.y),
    width: Math.round(node.width),
    height: Math.round(node.height),
  }

  // qt-solid metadata
  const comp = node.getSharedPluginData(QT_SOLID_NS, KEY_COMPONENT)
  if (comp) data.qtSolidComponent = comp
  const file = node.getSharedPluginData(QT_SOLID_NS, KEY_FILE)
  if (file) data.qtSolidFile = file

  // Variant info for instances
  if (node.type === "INSTANCE") {
    data.variantProperties = (node as InstanceNode).variantProperties
  }

  return data
}

function sendSelection(): void {
  const nodes = figma.currentPage.selection.map(serializeNode)
  figma.ui.postMessage({ type: "selection", nodes })
}

figma.on("selectionchange", sendSelection)

// ---------------------------------------------------------------------------
// Document changes → UI
// ---------------------------------------------------------------------------

figma.on("documentchange", (event) => {
  const changes = event.documentChanges.map((change) => ({
    type: change.type,
    origin: change.origin,
    id: "id" in change ? (change as any).id : undefined,
    properties: change.type === "PROPERTY_CHANGE"
      ? (change as PropertyChange).properties
      : undefined,
  }))
  figma.ui.postMessage({ type: "document-change", changes })
})

// ---------------------------------------------------------------------------
// UI → Plugin commands
// ---------------------------------------------------------------------------

interface TagComponentMsg {
  type: "tag-component"
  nodeId: string
  exportName: string
  filePath: string
}

interface ReadTokensMsg {
  type: "read-tokens"
}

interface WriteTokensMsg {
  type: "write-tokens"
  tokens: Array<{
    name: string
    type: "COLOR" | "FLOAT" | "STRING" | "BOOLEAN"
    value: unknown
    collectionName?: string
  }>
}

interface GenerateCodeMsg {
  type: "generate-code"
  nodeId: string
}

interface ReadNodeTreeMsg {
  type: "read-node-tree"
  nodeId: string
}

type UIMessage =
  | TagComponentMsg
  | ReadTokensMsg
  | WriteTokensMsg
  | GenerateCodeMsg
  | ReadNodeTreeMsg

figma.ui.onmessage = async (msg: UIMessage) => {
  switch (msg.type) {
    case "tag-component":
      await handleTagComponent(msg)
      break
    case "read-tokens":
      await handleReadTokens()
      break
    case "write-tokens":
      await handleWriteTokens(msg)
      break
    case "generate-code":
      await handleGenerateCode(msg)
      break
    case "read-node-tree":
      await handleReadNodeTree(msg)
      break
  }
}

// ---------------------------------------------------------------------------
// Tag component — mark a Figma node as linked to a qt-solid component
// ---------------------------------------------------------------------------

async function handleTagComponent(msg: TagComponentMsg): Promise<void> {
  const node = await figma.getNodeByIdAsync(msg.nodeId)
  if (!node || !("setSharedPluginData" in node)) {
    figma.ui.postMessage({ type: "error", message: `Node ${msg.nodeId} not found` })
    return
  }
  const sceneNode = node as SceneNode
  sceneNode.setSharedPluginData(QT_SOLID_NS, KEY_COMPONENT, msg.exportName)
  sceneNode.setSharedPluginData(QT_SOLID_NS, KEY_FILE, msg.filePath)
  figma.ui.postMessage({
    type: "tag-result",
    nodeId: msg.nodeId,
    exportName: msg.exportName,
    filePath: msg.filePath,
  })
}

// ---------------------------------------------------------------------------
// Read tokens — export all local variables as design tokens
// ---------------------------------------------------------------------------

async function handleReadTokens(): Promise<void> {
  const collections = await figma.variables.getLocalVariableCollectionsAsync()
  const tokens: Array<{
    collection: string
    name: string
    type: string
    values: Record<string, unknown>
  }> = []

  for (const collection of collections) {
    const modeNames: Record<string, string> = {}
    for (const mode of collection.modes) {
      modeNames[mode.modeId] = mode.name
    }

    for (const varId of collection.variableIds) {
      const variable = await figma.variables.getVariableByIdAsync(varId)
      if (!variable) continue

      const values: Record<string, unknown> = {}
      for (const [modeId, val] of Object.entries(variable.valuesByMode)) {
        const modeName = modeNames[modeId] ?? modeId
        if (typeof val === "object" && val !== null && "type" in val) {
          // VARIABLE_ALIAS — resolve
          const alias = val as { type: "VARIABLE_ALIAS"; id: string }
          if (alias.type === "VARIABLE_ALIAS") {
            const target = await figma.variables.getVariableByIdAsync(alias.id)
            values[modeName] = target ? `{${target.name}}` : alias.id
            continue
          }
        }
        values[modeName] = val
      }

      tokens.push({
        collection: collection.name,
        name: variable.name,
        type: variable.resolvedType,
        values,
      })
    }
  }

  figma.ui.postMessage({ type: "tokens", tokens })
}

// ---------------------------------------------------------------------------
// Write tokens — import tokens into Figma variables
// ---------------------------------------------------------------------------

async function handleWriteTokens(msg: WriteTokensMsg): Promise<void> {
  const collections = await figma.variables.getLocalVariableCollectionsAsync()
  let created = 0
  let updated = 0

  for (const token of msg.tokens) {
    const collectionName = token.collectionName ?? "qt-solid tokens"
    let collection = collections.find(c => c.name === collectionName)
    if (!collection) {
      collection = figma.variables.createVariableCollection(collectionName)
      collections.push(collection)
    }

    const existingVars: Variable[] = []
    for (const varId of collection.variableIds) {
      const v = await figma.variables.getVariableByIdAsync(varId)
      if (v) existingVars.push(v)
    }

    let variable = existingVars.find(v => v.name === token.name)
    const defaultModeId = collection.modes[0]?.modeId
    if (!defaultModeId) continue

    if (!variable) {
      variable = figma.variables.createVariable(token.name, collection, token.type)
      created++
    } else {
      updated++
    }

    variable.setValueForMode(defaultModeId, token.value as VariableValue)
  }

  figma.ui.postMessage({
    type: "write-tokens-result",
    created,
    updated,
  })
}

// ---------------------------------------------------------------------------
// Generate code — read a node tree and send to UI for codegen
// ---------------------------------------------------------------------------

async function handleGenerateCode(msg: GenerateCodeMsg): Promise<void> {
  const node = await figma.getNodeByIdAsync(msg.nodeId)
  if (!node || !("type" in node)) {
    figma.ui.postMessage({ type: "error", message: `Node ${msg.nodeId} not found` })
    return
  }
  // Send full node tree to UI; actual codegen happens in the iframe
  // where we can use the convert logic
  const tree = await serializeNodeDeep(node as SceneNode)
  figma.ui.postMessage({ type: "node-tree", tree })
}

// ---------------------------------------------------------------------------
// Read node tree — deep serialization for preview/codegen
// ---------------------------------------------------------------------------

async function handleReadNodeTree(msg: ReadNodeTreeMsg): Promise<void> {
  const node = await figma.getNodeByIdAsync(msg.nodeId)
  if (!node || !("type" in node)) {
    figma.ui.postMessage({ type: "error", message: `Node ${msg.nodeId} not found` })
    return
  }
  const tree = await serializeNodeDeep(node as SceneNode)
  figma.ui.postMessage({ type: "node-tree", tree })
}

async function serializeNodeDeep(node: SceneNode): Promise<Record<string, unknown>> {
  const data = serializeNode(node)

  // Layout
  if ("layoutMode" in node) {
    const frame = node as FrameNode
    data.layoutMode = frame.layoutMode
    data.primaryAxisAlignItems = frame.primaryAxisAlignItems
    data.counterAxisAlignItems = frame.counterAxisAlignItems
    data.itemSpacing = frame.itemSpacing
    data.paddingLeft = frame.paddingLeft
    data.paddingRight = frame.paddingRight
    data.paddingTop = frame.paddingTop
    data.paddingBottom = frame.paddingBottom
  }

  // Fills
  if ("fills" in node) {
    const fills = (node as GeometryMixin).fills
    if (fills !== figma.mixed && Array.isArray(fills)) {
      data.fills = fills.map(f => ({
        type: f.type,
        visible: f.visible,
        color: f.type === "SOLID" ? f.color : undefined,
        opacity: f.type === "SOLID" ? f.opacity : undefined,
      }))
    }
  }

  // Children
  if ("children" in node) {
    const children: Record<string, unknown>[] = []
    for (const child of (node as FrameNode).children) {
      children.push(await serializeNodeDeep(child))
    }
    data.children = children
  }

  return data
}

// Send initial selection
sendSelection()
