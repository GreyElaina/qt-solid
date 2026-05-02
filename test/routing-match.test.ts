import { describe, it, expect } from "vitest";
import { matchRoutes } from "../packages/solid/src/app/routing/match.ts";
import type { RouteDefinition } from "../packages/solid/src/app/routing/types.ts";

const Noop = () => undefined;

describe("matchRoutes", () => {
  const routes: RouteDefinition[] = [
    { path: "/", component: Noop },
    {
      path: "/settings",
      component: Noop,
      children: [
        { path: "/general", component: Noop },
        { path: "/accounts", component: Noop },
        { path: "/about", component: Noop },
      ],
    },
    { path: "/users/:id", component: Noop },
    { path: "/files/*path", component: Noop },
  ];

  it("matches root", () => {
    const result = matchRoutes(routes, "/");
    expect(result).toHaveLength(1);
    expect(result![0]!.route.path).toBe("/");
  });

  it("matches nested route", () => {
    const result = matchRoutes(routes, "/settings/general");
    expect(result).toHaveLength(2);
    expect(result![0]!.route.path).toBe("/settings");
    expect(result![1]!.route.path).toBe("/general");
  });

  it("extracts params", () => {
    const result = matchRoutes(routes, "/users/42");
    expect(result).toHaveLength(1);
    expect(result![0]!.params).toEqual({ id: "42" });
  });

  it("captures wildcard", () => {
    const result = matchRoutes(routes, "/files/docs/readme.md");
    expect(result).toHaveLength(1);
    expect(result![0]!.params).toEqual({ path: "/docs/readme.md" });
  });

  it("returns null for no match", () => {
    expect(matchRoutes(routes, "/nonexistent")).toBeNull();
  });

  it("matches parent with component when no child matches", () => {
    const result = matchRoutes(routes, "/settings");
    expect(result).toHaveLength(1);
    expect(result![0]!.route.path).toBe("/settings");
  });
});
