import type { Accessor, Component, JSX } from "solid-js";

/** A single route definition in the tree. */
export interface RouteDefinition {
  /** Path segment pattern, e.g. "/settings", "/:id", "/*rest". */
  path: string;
  /** Component to render when this route matches. */
  component?: Component;
  /** Nested child routes. */
  children?: RouteDefinition[];
}

/** One level of a matched branch (root → leaf chain). */
export interface BranchEntry {
  route: RouteDefinition;
  params: Record<string, string>;
  /** Remaining unmatched path for child routes. */
  remaining: string;
}

/** A snapshot of the navigation stack. */
export interface StackEntry {
  path: string;
  /** Unique key for keyed presence transitions. */
  key: number;
}

/** Navigation operations. */
export interface NavigateFn {
  push(path: string): void;
  pop(): void;
  replace(path: string): void;
}

export interface RouterContextState {
  location: Accessor<string>;
  stack: Accessor<readonly StackEntry[]>;
  navigate: NavigateFn;
  /** Matched branch for the current location, from root to leaf. */
  branch: Accessor<readonly BranchEntry[]>;
  routes: RouteDefinition[];
}

export interface OutletDepthState {
  depth: number;
}
