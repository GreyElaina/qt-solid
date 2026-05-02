import type { BranchEntry, RouteDefinition } from "./types.ts";

/**
 * Match a single path segment pattern against an actual segment.
 * Returns extracted param or null on mismatch.
 */
function matchSegment(
  pattern: string,
  actual: string,
): { param?: [string, string] } | null {
  if (pattern.startsWith(":")) {
    const name = pattern.slice(1);
    // Dynamic segment — always matches a non-empty segment
    if (actual.length === 0) return null;
    return { param: [name, actual] };
  }
  // Static segment — exact match
  if (pattern === actual) return {};
  return null;
}

/**
 * Normalize a path into non-empty segments.
 * "/settings/general" → ["settings", "general"]
 * "/" → []
 */
function toSegments(path: string): string[] {
  return path.split("/").filter((s) => s.length > 0);
}

/**
 * Recursively match a path against a route tree.
 * Returns the branch (root → leaf) on success, or null on no match.
 *
 * Matching semantics:
 * - A route with children acts as a prefix match.
 * - A leaf route must consume all remaining segments (unless it ends with /*).
 * - Wildcard `/*rest` captures remaining path as param "rest".
 */
export function matchRoutes(
  routes: RouteDefinition[],
  path: string,
): BranchEntry[] | null {
  const segments = toSegments(path);
  return matchRoutesImpl(routes, segments);
}

function matchRoutesImpl(
  routes: RouteDefinition[],
  segments: string[],
): BranchEntry[] | null {
  for (const route of routes) {
    const result = tryMatch(route, segments);
    if (result) return result;
  }
  return null;
}

function tryMatch(
  route: RouteDefinition,
  segments: string[],
): BranchEntry[] | null {
  const patternSegments = toSegments(route.path);
  const params: Record<string, string> = {};
  let consumed = 0;

  for (let i = 0; i < patternSegments.length; i++) {
    const pat = patternSegments[i]!;

    // Wildcard — captures rest
    if (pat.startsWith("*")) {
      const name = pat.slice(1) || "rest";
      params[name] = "/" + segments.slice(consumed).join("/");
      const entry: BranchEntry = {
        route,
        params,
        remaining: "",
      };
      return [entry];
    }

    if (consumed >= segments.length) {
      // Pattern expects more segments than available
      return null;
    }

    const m = matchSegment(pat, segments[consumed]!);
    if (!m) return null;
    if (m.param) {
      params[m.param[0]] = m.param[1];
    }
    consumed++;
  }

  const remaining = segments.slice(consumed);
  const remainingPath =
    remaining.length > 0 ? "/" + remaining.join("/") : "";

  // Has children — delegate remaining to children
  if (route.children && route.children.length > 0) {
    // Allow parent to match even when remaining is empty (index route)
    const childBranch = remaining.length > 0
      ? matchRoutesImpl(route.children, remaining)
      : matchRoutesImpl(route.children, []);

    const entry: BranchEntry = {
      route,
      params,
      remaining: remainingPath,
    };

    if (childBranch) {
      return [entry, ...childBranch];
    }

    // If no child matched but parent has a component, render parent alone
    // only when all segments are consumed
    if (remaining.length === 0 && route.component) {
      return [entry];
    }

    return null;
  }

  // Leaf route — must consume all segments
  if (remaining.length > 0) return null;

  return [
    {
      route,
      params,
      remaining: "",
    },
  ];
}
