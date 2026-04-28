export interface FluentIconDef {
  readonly d: string
  /** Coordinate space width (and height) the path is authored in. */
  readonly viewBox: number
}

// Hand-authored simplified Fluent-style icon paths in a 16x16 coordinate space.
// These approximate the WinUI 3 / Segoe Fluent Icons visual language.

export const FluentIcons = {
  Close: {
    d: "M 3.5 3.5 L 12.5 12.5 M 12.5 3.5 L 3.5 12.5",
    viewBox: 16,
  },
  Accept: {
    d: "M 2.5 8.5 L 6 12 L 13.5 4.5",
    viewBox: 16,
  },
  Search: {
    d: "M 10.5 10.5 L 14 14 M 7 3 A 4 4 0 1 1 7 11 A 4 4 0 1 1 7 3",
    viewBox: 16,
  },
  Add: {
    d: "M 8 3 L 8 13 M 3 8 L 13 8",
    viewBox: 16,
  },
  Info: {
    d: "M 8 4 L 8 4.01 M 8 7 L 8 12",
    viewBox: 16,
  },
  Warning: {
    d: "M 8 3 L 14 13 L 2 13 Z M 8 7 L 8 10 M 8 11.5 L 8 11.51",
    viewBox: 16,
  },
  Error: {
    d: "M 8 1 A 7 7 0 1 0 8 15 A 7 7 0 1 0 8 1 M 5.5 5.5 L 10.5 10.5 M 10.5 5.5 L 5.5 10.5",
    viewBox: 16,
  },
  Delete: {
    d: "M 5 3 L 5 2 L 11 2 L 11 3 M 3 3 L 13 3 M 4.5 3 L 5 14 L 11 14 L 11.5 3 M 7 5.5 L 7 11.5 M 9 5.5 L 9 11.5",
    viewBox: 16,
  },
  Edit: {
    d: "M 2 14 L 3 10 L 11 2 L 14 5 L 6 13 Z M 10 3 L 13 6",
    viewBox: 16,
  },
  Settings: {
    d: "M 7 1 L 9 1 L 9.5 3 L 11.5 4 L 13.5 3 L 15 4.5 L 13.5 6.5 L 14 8.5 L 16 9 L 16 11 L 14 11.5 L 13.5 13.5 L 15 15 L 13.5 16.5 L 11.5 15 L 9.5 15.5 L 9 17 L 7 17 L 6.5 15 L 4.5 14.5 L 2.5 15.5 L 1 14 L 2.5 12 L 2 10 L 0 9.5 L 0 7.5 L 2 7 L 2.5 5 L 1 3 L 2.5 1.5 L 4.5 3 L 6.5 2.5 Z M 8 6 A 3 3 0 1 0 8 12 A 3 3 0 1 0 8 6",
    viewBox: 17,
  },
  Home: {
    d: "M 8 2 L 1 8 L 3 8 L 3 14 L 6.5 14 L 6.5 10 L 9.5 10 L 9.5 14 L 13 14 L 13 8 L 15 8 Z",
    viewBox: 16,
  },
  ChevronDown: {
    d: "M 3 6 L 8 11 L 13 6",
    viewBox: 16,
  },
  ChevronUp: {
    d: "M 3 11 L 8 6 L 13 11",
    viewBox: 16,
  },
  ChevronLeft: {
    d: "M 10 3 L 5 8 L 10 13",
    viewBox: 16,
  },
  ChevronRight: {
    d: "M 6 3 L 11 8 L 6 13",
    viewBox: 16,
  },
} as const satisfies Record<string, FluentIconDef>

export type FluentIconName = keyof typeof FluentIcons
