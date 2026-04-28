// Qt::Key → DOM KeyboardEvent.key mapping for non-printable/functional keys.
// Reference: https://www.w3.org/TR/uievents-key/
// Qt::Key values: https://doc.qt.io/qt-6/qt.html#Key-enum

const QT_KEY_TO_DOM_KEY: Record<number, string> = {
  // Navigation
  0x01000012: "ArrowLeft",   // Qt::Key_Left
  0x01000013: "ArrowUp",     // Qt::Key_Up
  0x01000014: "ArrowRight",  // Qt::Key_Right
  0x01000015: "ArrowDown",   // Qt::Key_Down
  0x01000010: "Home",        // Qt::Key_Home
  0x01000011: "End",         // Qt::Key_End
  0x01000016: "PageUp",      // Qt::Key_PageUp
  0x01000017: "PageDown",    // Qt::Key_PageDown

  // Editing
  0x01000000: "Escape",      // Qt::Key_Escape
  0x01000001: "Tab",         // Qt::Key_Tab
  0x01000002: "Tab",         // Qt::Key_Backtab (Shift+Tab still "Tab")
  0x01000003: "Enter",       // Qt::Key_Return
  0x01000004: "Enter",       // Qt::Key_Enter (numpad)
  0x01000005: "Insert",      // Qt::Key_Insert
  0x01000006: "Backspace",   // Qt::Key_Backspace
  0x01000007: "Delete",      // Qt::Key_Delete

  // Modifiers
  0x01000020: "Shift",       // Qt::Key_Shift
  0x01000021: "Control",     // Qt::Key_Control
  0x01000022: "Meta",        // Qt::Key_Meta (Command on macOS)
  0x01000023: "Alt",         // Qt::Key_Alt
  0x01001103: "AltGraph",    // Qt::Key_AltGr
  0x01000024: "CapsLock",    // Qt::Key_CapsLock
  0x01000025: "NumLock",     // Qt::Key_NumLock
  0x01000026: "ScrollLock",  // Qt::Key_ScrollLock

  // Function keys
  0x01000030: "F1",
  0x01000031: "F2",
  0x01000032: "F3",
  0x01000033: "F4",
  0x01000034: "F5",
  0x01000035: "F6",
  0x01000036: "F7",
  0x01000037: "F8",
  0x01000038: "F9",
  0x01000039: "F10",
  0x0100003a: "F11",
  0x0100003b: "F12",
  0x0100003c: "F13",
  0x0100003d: "F14",
  0x0100003e: "F15",
  0x0100003f: "F16",
  0x01000040: "F17",
  0x01000041: "F18",
  0x01000042: "F19",
  0x01000043: "F20",

  // System
  0x01000060: "PrintScreen", // Qt::Key_Print
  0x01000008: "Pause",       // Qt::Key_Pause
  0x0100000a: "ContextMenu", // Qt::Key_Menu

  // Media
  0x01000070: "AudioVolumeUp",    // Qt::Key_VolumeUp
  0x01000071: "AudioVolumeDown",  // Qt::Key_VolumeDown
  0x01000072: "AudioVolumeMute",  // Qt::Key_VolumeMute

  // Space (text is " " but key name is fixed)
  0x20: " ",
}

// macOS virtual key codes (from NSEvent.keyCode / Carbon Events.h)
// → DOM KeyboardEvent.code
// Reference: https://w3c.github.io/uievents-code/
const MACOS_VIRTUAL_KEY_TO_DOM_CODE: Record<number, string> = {
  0x00: "KeyA",
  0x01: "KeyS",
  0x02: "KeyD",
  0x03: "KeyF",
  0x04: "KeyH",
  0x05: "KeyG",
  0x06: "KeyZ",
  0x07: "KeyX",
  0x08: "KeyC",
  0x09: "KeyV",
  0x0A: "IntlBackslash",
  0x0B: "KeyB",
  0x0C: "KeyQ",
  0x0D: "KeyW",
  0x0E: "KeyE",
  0x0F: "KeyR",
  0x10: "KeyY",
  0x11: "KeyT",
  0x12: "Digit1",
  0x13: "Digit2",
  0x14: "Digit3",
  0x15: "Digit4",
  0x16: "Digit6",
  0x17: "Digit5",
  0x18: "Equal",
  0x19: "Digit9",
  0x1A: "Digit7",
  0x1B: "Minus",
  0x1C: "Digit8",
  0x1D: "Digit0",
  0x1E: "BracketRight",
  0x1F: "KeyO",
  0x20: "KeyU",
  0x21: "BracketLeft",
  0x22: "KeyI",
  0x23: "KeyP",
  0x24: "Enter",
  0x25: "KeyL",
  0x26: "KeyJ",
  0x27: "Quote",
  0x28: "KeyK",
  0x29: "Semicolon",
  0x2A: "Backslash",
  0x2B: "Comma",
  0x2C: "Slash",
  0x2D: "KeyN",
  0x2E: "KeyM",
  0x2F: "Period",
  0x30: "Tab",
  0x31: "Space",
  0x32: "Backquote",
  0x33: "Backspace",
  0x35: "Escape",
  0x36: "MetaRight",
  0x37: "MetaLeft",
  0x38: "ShiftLeft",
  0x39: "CapsLock",
  0x3A: "AltLeft",
  0x3B: "ControlLeft",
  0x3C: "ShiftRight",
  0x3D: "AltRight",
  0x3E: "ControlRight",
  0x3F: "Fn",
  0x40: "F17",
  0x41: "NumpadDecimal",
  0x43: "NumpadMultiply",
  0x45: "NumpadAdd",
  0x47: "NumLock",
  0x4B: "NumpadDivide",
  0x4C: "NumpadEnter",
  0x4E: "NumpadSubtract",
  0x4F: "F18",
  0x50: "F19",
  0x51: "NumpadEqual",
  0x52: "Numpad0",
  0x53: "Numpad1",
  0x54: "Numpad2",
  0x55: "Numpad3",
  0x56: "Numpad4",
  0x57: "Numpad5",
  0x58: "Numpad6",
  0x59: "Numpad7",
  0x5A: "F20",
  0x5B: "Numpad8",
  0x5C: "Numpad9",
  0x5D: "IntlYen",
  0x5E: "IntlRo",
  0x60: "F5",
  0x61: "F6",
  0x62: "F7",
  0x63: "F3",
  0x64: "F8",
  0x65: "F9",
  0x67: "F11",
  0x69: "F13",
  0x6A: "F16",
  0x6B: "F14",
  0x6D: "F10",
  0x6F: "F12",
  0x71: "F15",
  0x72: "Help",
  0x73: "Home",
  0x74: "PageUp",
  0x75: "Delete",
  0x76: "F4",
  0x77: "End",
  0x78: "F2",
  0x79: "PageDown",
  0x7A: "F1",
  0x7B: "ArrowLeft",
  0x7C: "ArrowRight",
  0x7D: "ArrowDown",
  0x7E: "ArrowUp",
}

// Windows scan code → DOM code (extended-key aware: 0xE0xx prefix)
const WINDOWS_SCAN_CODE_TO_DOM_CODE: Record<number, string> = {
  0x01: "Escape",
  0x02: "Digit1",
  0x03: "Digit2",
  0x04: "Digit3",
  0x05: "Digit4",
  0x06: "Digit5",
  0x07: "Digit6",
  0x08: "Digit7",
  0x09: "Digit8",
  0x0A: "Digit9",
  0x0B: "Digit0",
  0x0C: "Minus",
  0x0D: "Equal",
  0x0E: "Backspace",
  0x0F: "Tab",
  0x10: "KeyQ",
  0x11: "KeyW",
  0x12: "KeyE",
  0x13: "KeyR",
  0x14: "KeyT",
  0x15: "KeyY",
  0x16: "KeyU",
  0x17: "KeyI",
  0x18: "KeyO",
  0x19: "KeyP",
  0x1A: "BracketLeft",
  0x1B: "BracketRight",
  0x1C: "Enter",
  0x1D: "ControlLeft",
  0x1E: "KeyA",
  0x1F: "KeyS",
  0x20: "KeyD",
  0x21: "KeyF",
  0x22: "KeyG",
  0x23: "KeyH",
  0x24: "KeyJ",
  0x25: "KeyK",
  0x26: "KeyL",
  0x27: "Semicolon",
  0x28: "Quote",
  0x29: "Backquote",
  0x2A: "ShiftLeft",
  0x2B: "Backslash",
  0x2C: "KeyZ",
  0x2D: "KeyX",
  0x2E: "KeyC",
  0x2F: "KeyV",
  0x30: "KeyB",
  0x31: "KeyN",
  0x32: "KeyM",
  0x33: "Comma",
  0x34: "Period",
  0x35: "Slash",
  0x36: "ShiftRight",
  0x37: "NumpadMultiply",
  0x38: "AltLeft",
  0x39: "Space",
  0x3A: "CapsLock",
  0x3B: "F1",
  0x3C: "F2",
  0x3D: "F3",
  0x3E: "F4",
  0x3F: "F5",
  0x40: "F6",
  0x41: "F7",
  0x42: "F8",
  0x43: "F9",
  0x44: "F10",
  0x45: "NumLock",
  0x46: "ScrollLock",
  0x47: "Numpad7",
  0x48: "Numpad8",
  0x49: "Numpad9",
  0x4A: "NumpadSubtract",
  0x4B: "Numpad4",
  0x4C: "Numpad5",
  0x4D: "Numpad6",
  0x4E: "NumpadAdd",
  0x4F: "Numpad1",
  0x50: "Numpad2",
  0x51: "Numpad3",
  0x52: "Numpad0",
  0x53: "NumpadDecimal",
  0x56: "IntlBackslash",
  0x57: "F11",
  0x58: "F12",
  0x59: "IntlRo",
  0x64: "F13",
  0x65: "F14",
  0x66: "F15",
  0x67: "F16",
  0x68: "F17",
  0x69: "F18",
  0x6A: "F19",
  0x6B: "F20",
  0x70: "KanaMode",
  0x73: "IntlRo",
  0x79: "Convert",
  0x7B: "NonConvert",
  0x7D: "IntlYen",
  0x7E: "NumpadComma",
  // Extended keys (0xE0 prefix)
  0xE01C: "NumpadEnter",
  0xE01D: "ControlRight",
  0xE035: "NumpadDivide",
  0xE037: "PrintScreen",
  0xE038: "AltRight",
  0xE045: "NumLock",
  0xE046: "Pause",
  0xE047: "Home",
  0xE048: "ArrowUp",
  0xE049: "PageUp",
  0xE04B: "ArrowLeft",
  0xE04D: "ArrowRight",
  0xE04F: "End",
  0xE050: "ArrowDown",
  0xE051: "PageDown",
  0xE052: "Insert",
  0xE053: "Delete",
  0xE05B: "MetaLeft",
  0xE05C: "MetaRight",
  0xE05D: "ContextMenu",
}

// Detect platform once.
const IS_MACOS = typeof navigator !== "undefined"
  ? navigator.platform?.startsWith("Mac") ?? false
  : typeof process !== "undefined"
    ? process.platform === "darwin"
    : false

function nativeToDomCode(
  nativeScanCode: number,
  nativeVirtualKey: number,
): string {
  if (IS_MACOS) {
    return MACOS_VIRTUAL_KEY_TO_DOM_CODE[nativeVirtualKey] ?? ""
  }
  return WINDOWS_SCAN_CODE_TO_DOM_CODE[nativeScanCode] ?? ""
}

function qtKeyToDomKey(qtKey: number, text: string): string {
  // Printable character available — use it directly.
  if (text.length > 0 && text.charCodeAt(0) >= 0x20) {
    return text
  }
  return QT_KEY_TO_DOM_KEY[qtKey] ?? "Unidentified"
}

export interface CanvasKeyboardEventPayload {
  key: string
  code: string
  repeat: boolean
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
  metaKey: boolean
}

// Qt modifier flags (from Qt::KeyboardModifier)
const QT_SHIFT   = 0x02000000
const QT_CONTROL = 0x04000000
const QT_ALT     = 0x08000000
const QT_META    = 0x10000000

export function buildCanvasKeyboardPayload(
  qtKey: number,
  modifiers: number,
  text: string,
  repeat: boolean,
  nativeScanCode: number,
  nativeVirtualKey: number,
): CanvasKeyboardEventPayload {
  return {
    key: qtKeyToDomKey(qtKey, text),
    code: nativeToDomCode(nativeScanCode, nativeVirtualKey),
    repeat,
    ctrlKey: (modifiers & QT_CONTROL) !== 0,
    shiftKey: (modifiers & QT_SHIFT) !== 0,
    altKey: (modifiers & QT_ALT) !== 0,
    metaKey: (modifiers & QT_META) !== 0,
  }
}
