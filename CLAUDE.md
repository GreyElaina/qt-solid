---
description: Use Node.js + Vite/Vitest for tooling in this repo.
globs: "*.ts, *.tsx, *.js, *.jsx, package.json, vitest.config.ts"
alwaysApply: false
---

Default to Node.js + Vite/Vitest in this repo.

- Use `node` for runtime scripts.
- Use `vitest` for tests.
- Use Vite plugin/build hooks for TSX transform work.
- Keep native build on `napi` + `cargo`.
- Do not assume Bun-specific transforms or `bun:test`.
