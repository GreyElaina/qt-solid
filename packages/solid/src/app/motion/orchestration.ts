import { createSignal, type Accessor } from "solid-js";

import type { MotionTransition } from "./types.ts";

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

const orchestrationByNode = new WeakMap<object, OrchestrationParentControl>();

function isOrchestratedNode(node: unknown): node is object & { parent: unknown } {
  return node != null
    && typeof node === "object"
    && "parent" in node;
}

export function hasChildStaggerOrchestration(
  transition: MotionTransition | undefined,
): boolean {
  return (transition?.delayChildren ?? 0) > 0
    || (transition?.staggerChildren ?? 0) > 0;
}

/**
 * Create orchestration state from a parent's transition config.
 */
export function createOrchestration(
  config: OrchestrationConfig,
): OrchestrationParentControl {
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
  };
}

/** Extended state including parent-only control. */
export type OrchestrationParentControl = OrchestrationContextState & {
  unlockChildren: () => void;
};

export function attachOrchestration(
  node: object,
  orchestration: OrchestrationParentControl,
): void {
  orchestrationByNode.set(node, orchestration);
}

export function detachOrchestration(node: object): void {
  orchestrationByNode.delete(node);
}

export function findParentOrchestration(
  node: unknown,
): OrchestrationContextState | undefined {
  let current = isOrchestratedNode(node) ? node.parent : null;
  while (isOrchestratedNode(current)) {
    const orchestration = orchestrationByNode.get(current);
    if (orchestration) {
      return orchestration;
    }
    current = current.parent;
  }
  return undefined;
}
