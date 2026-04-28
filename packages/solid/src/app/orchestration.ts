import {
  createContext,
  createSignal,
  useContext,
  type Accessor,
} from "solid-js";

export interface OrchestrationConfig {
  delayChildren: number;
  staggerChildren: number;
  when: "beforeChildren" | "afterChildren" | false;
}

export interface OrchestrationContextState {
  /** Delay for this child = delayChildren + index * staggerChildren. */
  getChildDelay: (index: number) => number;
  /** Register a child, returns its stagger index. */
  registerChild: () => number;
  /**
   * When `when` is set, children wait for this to become true before animating.
   * - `when: "beforeChildren"` → becomes true after parent animation completes.
   * - `when: false` or `when: "afterChildren"` → starts as true (no gate).
   */
  childrenCanAnimate: Accessor<boolean>;
  /**
   * Signal that a child's animation completed.
   * Used for `when: "afterChildren"` — parent waits for all children.
   */
  onChildComplete: () => void;
  /**
   * For `when: "afterChildren"` — becomes true when all registered children complete.
   */
  allChildrenComplete: Accessor<boolean>;
}

export const OrchestrationContext = createContext<OrchestrationContextState>();

export function useOrchestration(): OrchestrationContextState | undefined {
  return useContext(OrchestrationContext);
}

/**
 * Create orchestration state from a parent's transition config.
 * Returns `undefined` if no orchestration is needed.
 */
export function createOrchestration(
  config: OrchestrationConfig,
): OrchestrationContextState {
  let childCount = 0;
  let completedCount = 0;

  const gateDefault = config.when !== "beforeChildren";
  const [childrenCanAnimate, setChildrenCanAnimate] = createSignal(gateDefault);
  const [allChildrenComplete, setAllChildrenComplete] = createSignal(false);

  return {
    getChildDelay(index: number) {
      return config.delayChildren + index * config.staggerChildren;
    },
    registerChild() {
      return childCount++;
    },
    childrenCanAnimate,
    onChildComplete() {
      completedCount++;
      if (childCount > 0 && completedCount >= childCount) {
        setAllChildrenComplete(true);
      }
    },
    allChildrenComplete,
    /** Called by parent when its own animation completes (for "beforeChildren"). */
    unlockChildren() {
      setChildrenCanAnimate(true);
    },
  } as OrchestrationContextState & { unlockChildren: () => void };
}

/** Extended state including parent-only control. */
export type OrchestrationParentControl = OrchestrationContextState & {
  unlockChildren: () => void;
};
