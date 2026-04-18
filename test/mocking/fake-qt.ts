export class FakeQtNode {
  id: number
  parent: FakeQtNode | null = null
  firstChild: FakeQtNode | null = null
  nextSibling: FakeQtNode | null = null
  kind: string
  text = ""
  title = ""
  width = 0
  height = 0
  minWidth = 0
  minHeight = 0
  flexGrow = 0
  flexShrink = 1
  enabled = true
  visible = true
  frameless = false
  transparentBackground = false
  alwaysOnTop = false
  placeholder = ""
  checked = false
  flexDirection = "column"
  alignItems = "stretch"
  justifyContent = "flex-start"
  gap = 0
  padding = 0
  fontFamily = ""
  fontPointSize = 12
  fontWeight = 400
  fontItalic = false
  focusPolicy = "no-focus"
  autoFocus = false
  rangeValue = 0
  rangeMinimum = 0
  rangeMaximum = 100
  rangeStep = 1
  rangePageStep = 10
  cursorPosition = 0
  selectionStart = 0
  selectionEnd = 0
  destroyed = false
  appliedProps: Array<{ method: string; value: string | number | boolean }> = []

  constructor(id: number, kind: string) {
    this.id = id
    this.kind = kind
  }

  get node() {
    return this
  }

  isTextNode() {
    return this.kind === "text"
  }

  insertChild(child: FakeQtNode, anchor?: FakeQtNode | null) {
    child.detach()
    child.parent = this

    if (!this.firstChild) {
      this.firstChild = child
      child.nextSibling = null
      return
    }

    if (!anchor) {
      let cursor = this.firstChild
      while (cursor.nextSibling) {
        cursor = cursor.nextSibling
      }
      cursor.nextSibling = child
      child.nextSibling = null
      return
    }

    if (this.firstChild === anchor) {
      child.nextSibling = anchor
      this.firstChild = child
      return
    }

    let cursor = this.firstChild
    while (cursor.nextSibling && cursor.nextSibling !== anchor) {
      cursor = cursor.nextSibling
    }

    child.nextSibling = anchor
    cursor.nextSibling = child
  }

  removeChild(child: FakeQtNode) {
    if (this.firstChild === child) {
      this.firstChild = child.nextSibling
      child.parent = null
      child.nextSibling = null
      return
    }

    let cursor = this.firstChild
    while (cursor?.nextSibling && cursor.nextSibling !== child) {
      cursor = cursor.nextSibling
    }

    if (!cursor?.nextSibling) {
      throw new Error("child not found")
    }

    cursor.nextSibling = child.nextSibling
    child.parent = null
    child.nextSibling = null
  }

  destroy() {
    this.destroyed = true

    let child = this.firstChild
    while (child) {
      child.destroy()
      child = child.nextSibling
    }
  }

  setText(value: string) {
    this.appliedProps.push({ method: "setText", value })
    this.text = value
  }

  setTitle(value: string) {
    this.appliedProps.push({ method: "setTitle", value })
    this.title = value
  }

  setWidth(value: number) {
    this.appliedProps.push({ method: "setWidth", value })
    this.width = value
  }

  setHeight(value: number) {
    this.appliedProps.push({ method: "setHeight", value })
    this.height = value
  }

  setMinWidth(value: number) {
    this.appliedProps.push({ method: "setMinWidth", value })
    this.minWidth = value
  }

  setMinHeight(value: number) {
    this.appliedProps.push({ method: "setMinHeight", value })
    this.minHeight = value
  }

  setGrow(value: number) {
    this.appliedProps.push({ method: "setGrow", value })
    this.flexGrow = value
  }

  setShrink(value: number) {
    this.appliedProps.push({ method: "setShrink", value })
    this.flexShrink = value
  }

  setEnabled(value: boolean) {
    this.appliedProps.push({ method: "setEnabled", value })
    this.enabled = value
  }

  setVisible(value: boolean) {
    this.appliedProps.push({ method: "setVisible", value })
    this.visible = value
  }

  setFrameless(value: boolean) {
    this.appliedProps.push({ method: "setFrameless", value })
    this.frameless = value
  }

  setTransparentBackground(value: boolean) {
    this.appliedProps.push({ method: "setTransparentBackground", value })
    this.transparentBackground = value
  }

  setAlwaysOnTop(value: boolean) {
    this.appliedProps.push({ method: "setAlwaysOnTop", value })
    this.alwaysOnTop = value
  }

  setPlaceholder(value: string) {
    this.appliedProps.push({ method: "setPlaceholder", value })
    this.placeholder = value
  }

  setChecked(value: boolean) {
    this.appliedProps.push({ method: "setChecked", value })
    this.checked = value
  }

  setDirection(value: string) {
    this.appliedProps.push({ method: "setDirection", value })
    this.flexDirection = value
  }

  setAlignItems(value: string) {
    this.appliedProps.push({ method: "setAlignItems", value })
    this.alignItems = value
  }

  setJustifyContent(value: string) {
    this.appliedProps.push({ method: "setJustifyContent", value })
    this.justifyContent = value
  }

  setGap(value: number) {
    this.appliedProps.push({ method: "setGap", value })
    this.gap = value
  }

  setPadding(value: number) {
    this.appliedProps.push({ method: "setPadding", value })
    this.padding = value
  }

  setFamily(value: string) {
    this.appliedProps.push({ method: "setFamily", value })
    this.fontFamily = value
  }

  setPointSize(value: number) {
    this.appliedProps.push({ method: "setPointSize", value })
    this.fontPointSize = value
  }

  setWeight(value: number) {
    this.appliedProps.push({ method: "setWeight", value })
    this.fontWeight = value
  }

  setItalic(value: boolean) {
    this.appliedProps.push({ method: "setItalic", value })
    this.fontItalic = value
  }

  setFocusPolicy(value: string) {
    this.appliedProps.push({ method: "setFocusPolicy", value })
    this.focusPolicy = value
  }

  setAutoFocus(value: boolean) {
    this.appliedProps.push({ method: "setAutoFocus", value })
    this.autoFocus = value
  }

  setValue(value: number) {
    this.appliedProps.push({ method: "setValue", value })
    this.rangeValue = value
  }

  setMinimum(value: number) {
    this.appliedProps.push({ method: "setMinimum", value })
    this.rangeMinimum = value
  }

  setMaximum(value: number) {
    this.appliedProps.push({ method: "setMaximum", value })
    this.rangeMaximum = value
  }

  setStep(value: number) {
    this.appliedProps.push({ method: "setStep", value })
    this.rangeStep = value
  }

  setPageStep(value: number) {
    this.appliedProps.push({ method: "setPageStep", value })
    this.rangePageStep = value
  }

  setCursorPosition(value: number) {
    this.appliedProps.push({ method: "setCursorPosition", value })
    this.cursorPosition = value
  }

  setSelectionStart(value: number) {
    this.appliedProps.push({ method: "setSelectionStart", value })
    this.selectionStart = value
  }

  setSelectionEnd(value: number) {
    this.appliedProps.push({ method: "setSelectionEnd", value })
    this.selectionEnd = value
  }

  private detach() {
    this.parent?.removeChild(this)
  }
}

export class QtWindow extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("window") as QtWindow
  }
}

export class QtView extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("view") as QtView
  }
}

export class QtGroup extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("group") as QtGroup
  }
}

export class QtLabel extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("label") as QtLabel
  }
}

export class QtButton extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("button") as QtButton
  }
}

export class QtInput extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("input") as QtInput
  }
}

export class QtCheck extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("check") as QtCheck
  }
}

export class QtText extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("text") as QtText
  }
}

export class QtSlider extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("slider") as QtSlider
  }
}

export class QtDoubleSpinBox extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("doubleSpinBox") as QtDoubleSpinBox
  }
}

export class QtBanner extends FakeQtNode {
  static create(app: FakeQtApp) {
    return app.createElement("label") as QtBanner
  }
}

const fakeWidgetEntityConstructors = {
  window: QtWindow,
  view: QtView,
  group: QtGroup,
  label: QtLabel,
  button: QtButton,
  input: QtInput,
  check: QtCheck,
  text: QtText,
  slider: QtSlider,
  doubleSpinBox: QtDoubleSpinBox,
} as const

export class FakeQtApp {
  root = new FakeQtNode(1, "root")
  nodes = new Map<number, FakeQtNode>([[1, this.root]])
  private nextId = 2

  shutdown() {}

  createElement(kind: string) {
    const Entity = fakeWidgetEntityConstructors[kind as keyof typeof fakeWidgetEntityConstructors] ?? FakeQtNode
    const node = new Entity(this.nextId++, kind)
    this.nodes.set(node.id, node)
    return node
  }

  createTextNode(value: string) {
    const node = new QtText(this.nextId++, "text")
    node.setText(value)
    this.nodes.set(node.id, node)
    return node
  }
}

export function allFakeNodes(app: FakeQtApp) {
  return [...app.nodes.values()]
}

export function fakeNodeByKind(app: FakeQtApp, kind: string) {
  return allFakeNodes(app).find((node) => node.kind === kind)
}
