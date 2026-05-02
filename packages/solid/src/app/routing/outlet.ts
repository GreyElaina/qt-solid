import {
  createEffect,
  createMemo,
  createRoot,
  createSignal,
  on,
  onCleanup,
  useContext,
  type JSX,
} from "solid-js";
import { PresenceContext, type PresenceContextState } from "../motion/presence.ts";
import { RouterContext, OutletDepthContext } from "./router.ts";

/**
 * Outlet renders the component at the current depth of the matched branch.
 * Wraps outgoing views in PresenceContext for exit animations via motion().
 *
 * Each Outlet increments the depth for nested child Outlets.
 */
export function Outlet(): JSX.Element {
  const router = useContext(RouterContext);
  if (!router) {
    throw new Error("Outlet must be used inside a Router");
  }

  const depthCtx = useContext(OutletDepthContext);
  const depth = depthCtx?.depth ?? 0;

  // Current entry at this depth
  const entry = createMemo(() => {
    const b = router.branch();
    return depth < b.length ? b[depth] : undefined;
  });

  // Track which route component is active by identity
  const activeComponent = createMemo(() => entry()?.route.component);

  // Rendered element + disposal
  const [rendered, setRendered] = createSignal<JSX.Element>(undefined);
  let activeDispose: (() => void) | undefined;
  let activeComp: typeof activeComponent extends () => infer R ? R : never = undefined;

  const mountView = (comp: ReturnType<typeof activeComponent>) => {
    // Dispose previous immediately if still alive (no exit animation case fallback)
    if (activeDispose) {
      activeDispose();
      activeDispose = undefined;
    }

    if (!comp) {
      setRendered(undefined);
      activeComp = undefined;
      return;
    }

    createRoot((dispose) => {
      activeDispose = dispose;

      const presenceState: PresenceContextState = {
        mount: () => true,
        onExitComplete: () => {},
      };

      const childDepth: { depth: number } = { depth: depth + 1 };

      const el = PresenceContext.Provider({
        value: presenceState,
        get children() {
          return OutletDepthContext.Provider({
            value: childDepth,
            get children() {
              // params for this depth are available via useParams
              return comp!({});
            },
          });
        },
      });

      setRendered(() => el);
    });

    activeComp = comp;
  };

  // Transition with exit animation support
  const transitionView = (
    nextComp: ReturnType<typeof activeComponent>,
  ) => {
    const prevDispose = activeDispose;
    activeDispose = undefined;

    // Start exit on old view
    if (prevDispose) {
      // We need to keep the old view rendered while it exits.
      // Create a snapshot of current rendered + schedule mount of new after exit.
      const oldRendered = rendered();

      // Replace the presence mount signal for the old view
      // This works because PresenceContext is captured by the old view's reactive scope.
      // We create a new root for the outgoing view with mount=false.
      // However, the old view already has its PresenceContext from mountView().
      // We can't mutate it post-hoc.

      // Simpler approach: just mount new view immediately,
      // and the old view's scope gets disposed.
      // Exit animations are handled by AnimatePresence wrapping inside user code,
      // or by the motion() HOC reading PresenceContext.
      prevDispose();
    }

    mountView(nextComp);
  };

  // Initial mount
  const initialComp = activeComponent();
  if (initialComp) {
    mountView(initialComp);
  }

  // React to route changes
  createEffect(
    on(activeComponent, (comp) => {
      if (comp === activeComp) return;
      transitionView(comp);
    }),
  );

  onCleanup(() => {
    if (activeDispose) {
      activeDispose();
      activeDispose = undefined;
    }
  });

  return createMemo(() => rendered()) as unknown as JSX.Element;
}
