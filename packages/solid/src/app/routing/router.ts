import {
  createContext,
  createMemo,
  createSignal,
  useContext,
  type Accessor,
  type Component,
  type JSX,
} from "solid-js";
import { matchRoutes } from "./match.ts";
import type {
  BranchEntry,
  NavigateFn,
  OutletDepthState,
  RouteDefinition,
  RouterContextState,
  StackEntry,
} from "./types.ts";

// -- Contexts --

export const RouterContext = createContext<RouterContextState>();
export const OutletDepthContext = createContext<OutletDepthState>({ depth: 0 });

// -- Router component --

export interface RouterProps {
  routes: RouteDefinition[];
  /** Initial location path. Defaults to "/". */
  initial?: string;
  children?: JSX.Element;
}

let nextStackKey = 0;

export const Router: Component<RouterProps> = (props) => {
  const initialPath = props.initial ?? "/";
  const [location, setLocation] = createSignal(initialPath);

  const initialEntry: StackEntry = {
    path: initialPath,
    key: nextStackKey++,
  };
  const [stack, setStack] = createSignal<readonly StackEntry[]>([
    initialEntry,
  ]);

  const navigate: NavigateFn = {
    push(path: string) {
      const entry: StackEntry = { path, key: nextStackKey++ };
      setStack((s) => [...s, entry]);
      setLocation(path);
    },
    pop() {
      setStack((s) => {
        if (s.length <= 1) return s;
        const next = s.slice(0, -1);
        setLocation(next[next.length - 1]!.path);
        return next;
      });
    },
    replace(path: string) {
      setStack((s) => {
        if (s.length === 0) {
          const entry: StackEntry = { path, key: nextStackKey++ };
          setLocation(path);
          return [entry];
        }
        const next = [...s];
        next[next.length - 1] = { path, key: nextStackKey++ };
        setLocation(path);
        return next;
      });
    },
  };

  const branch = createMemo<readonly BranchEntry[]>(() => {
    return matchRoutes(props.routes, location()) ?? [];
  });

  const ctx: RouterContextState = {
    location,
    stack,
    navigate,
    branch,
    routes: props.routes,
  };

  return RouterContext.Provider({
    value: ctx,
    get children() {
      return props.children;
    },
  }) as unknown as JSX.Element;
};
