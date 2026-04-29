import {
  createContext,
  createEffect,
  createMemo,
  createSignal,
  createRoot,
  on,
  onCleanup,
  useContext,
  type Accessor,
  type Component,
  type JSX,
} from "solid-js";

// -- PresenceContext --

export interface PresenceContextState {
  /** false when the component should play its exit animation. */
  mount: Accessor<boolean>;
  /** Signal exit animation completion to AnimatePresence. */
  onExitComplete: () => void;
}

export const PresenceContext = createContext<PresenceContextState>();

export function usePresence(): PresenceContextState | undefined {
  return useContext(PresenceContext);
}

// -- AnimatePresence --

/**
 * Controls child mounting with exit animation support.
 *
 * AnimatePresence owns the child lifecycle — it renders children when
 * `when` is truthy and keeps them alive during exit animations.
 *
 * ```tsx
 * <AnimatePresence when={show()}>
 *   {() => (
 *     <motion(View)
 *       initial={{ opacity: 0 }}
 *       animate={{ opacity: 1 }}
 *       exit={{ opacity: 0 }}
 *     />
 *   )}
 * </AnimatePresence>
 * ```
 *
 * For list-level presence (multiple children), wrap each item individually.
 */
export const AnimatePresence: Component<{
  when: boolean;
  children: () => JSX.Element;
}> = (props) => {
  // State machine:
  //   when=true  → phase=mounted, render children with mount=true
  //   when→false → phase=exiting, keep children, set mount=false, wait for onExitComplete
  //   onExitComplete → phase=unmounted, dispose children scope

  type Phase = "unmounted" | "mounted" | "exiting";

  const [phase, setPhase] = createSignal<Phase>(
    props.when ? "mounted" : "unmounted",
  );
  const [mount, setMount] = createSignal(props.when);

  // The rendered child and its disposal handle.
  // We use createRoot to own the child's reactive scope so we can
  // dispose it on our terms, not Solid's.
  let childDispose: (() => void) | undefined;
  const [childElement, setChildElement] = createSignal<JSX.Element>(undefined);

  const contextValue: PresenceContextState = {
    mount,
    onExitComplete() {
      // Exit animation done — actually tear down
      setPhase("unmounted");
      setMount(false);
      if (childDispose) {
        childDispose();
        childDispose = undefined;
      }
      setChildElement(undefined);
    },
  };

  const mountChild = () => {
    // Dispose previous if somehow still alive
    if (childDispose) {
      childDispose();
      childDispose = undefined;
    }
    createRoot((dispose) => {
      childDispose = dispose;
      const el = PresenceContext.Provider({
        value: contextValue,
        get children() {
          return props.children();
        },
      });
      setChildElement(() => el);
    });
    setMount(true);
    setPhase("mounted");
  };

  const startExit = () => {
    setPhase("exiting");
    setMount(false);
    // Child stays rendered — motion component reads mount()=false and
    // triggers exit animation. When done it calls onExitComplete().
    // Safety: if there's no exit animation, the motion component calls
    // onExitComplete() synchronously (or there's no motion component
    // at all). Handle that by scheduling a fallback.
  };

  // Initial mount if when starts true
  if (props.when) {
    mountChild();
  }

  // React to when changes
  createEffect(
    on(
      () => props.when,
      (when) => {
        const current = phase();
        if (when && current === "unmounted") {
          mountChild();
        } else if (when && current === "exiting") {
          // Re-mount: abort exit, go back to mounted
          setMount(true);
          setPhase("mounted");
        } else if (!when && current === "mounted") {
          startExit();
        }
        // !when && unmounted/exiting → noop
      },
    ),
  );

  // Cleanup on component disposal
  onCleanup(() => {
    if (childDispose) {
      childDispose();
      childDispose = undefined;
    }
  });

  return createMemo(() => childElement()) as unknown as JSX.Element; // Accessor<T> !== QtRendererNode
};
