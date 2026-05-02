import { createMemo, useContext, type Accessor } from "solid-js";
import { RouterContext, OutletDepthContext } from "./router.ts";
import type { NavigateFn, StackEntry } from "./types.ts";

/** Current location path as a reactive accessor. */
export function useLocation(): Accessor<string> {
  const ctx = useContext(RouterContext);
  if (!ctx) throw new Error("useLocation must be used inside a Router");
  return ctx.location;
}

/** Navigation operations: push / pop / replace. */
export function useNavigate(): NavigateFn {
  const ctx = useContext(RouterContext);
  if (!ctx) throw new Error("useNavigate must be used inside a Router");
  return ctx.navigate;
}

/** Reactive params extracted from the current route match at this Outlet depth. */
export function useParams(): Accessor<Record<string, string>> {
  const ctx = useContext(RouterContext);
  const depthCtx = useContext(OutletDepthContext);
  if (!ctx) throw new Error("useParams must be used inside a Router");

  const depth = depthCtx?.depth ?? 0;
  // Params come from the branch entry *above* the current depth,
  // because the Outlet at depth N renders the component from branch[N],
  // but that component's own params are in branch[N].params.
  // However the component is mounted by the Outlet at depth N,
  // so inside the component the OutletDepthContext is depth+1.
  // Therefore we look at branch[depth - 1] when depth > 0, or branch[0].
  const entryDepth = depth > 0 ? depth - 1 : 0;

  return createMemo(() => {
    const b = ctx.branch();
    if (entryDepth < b.length) return b[entryDepth]!.params;
    return {};
  });
}

/** Whether the navigation stack has entries to go back to. */
export function useCanGoBack(): Accessor<boolean> {
  const ctx = useContext(RouterContext);
  if (!ctx) throw new Error("useCanGoBack must be used inside a Router");
  return createMemo(() => ctx.stack().length > 1);
}

/** The full navigation stack as a reactive accessor. */
export function useStack(): Accessor<readonly StackEntry[]> {
  const ctx = useContext(RouterContext);
  if (!ctx) throw new Error("useStack must be used inside a Router");
  return ctx.stack;
}

/**
 * Breadcrumb segments derived from the current location.
 * E.g. "/settings/general" → [{ label: "settings", path: "/settings" }, { label: "general", path: "/settings/general" }]
 */
export function useBreadcrumbs(): Accessor<
  readonly { label: string; path: string }[]
> {
  const location = useLocation();
  return createMemo(() => {
    const segments = location()
      .split("/")
      .filter((s) => s.length > 0);
    let accumulated = "";
    return segments.map((seg) => {
      accumulated += "/" + seg;
      return { label: seg, path: accumulated };
    });
  });
}
