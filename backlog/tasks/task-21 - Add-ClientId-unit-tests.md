---
id: TASK-21
title: Add ClientId unit tests
status: Done
assignee: []
created_date: '2026-03-16'
updated_date: '2026-03-16'
labels:
  - testing
  - coverage
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`ClientId` in `crates/net/src/server/client_id.rs` has only 33 % line coverage.
The uncovered lines are:

- `ClientId::get()` — the `const fn` that returns the inner `u64`
- `impl Display for ClientId` — `format!("{}", id)` → `"client#N"`
- Sequential ID generation — two consecutive `ClientId::new()` calls should produce
  values that differ by exactly 1

Adding a small inline test module will cover all 6 missed lines and bring this file
to 100 % coverage.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Test `client_id_get_returns_inner` — `ClientId::new().get()` returns a non-zero `u64`
- [ ] #2 Test `client_id_display` — `format!("{}", id)` produces the string `"client#N"` where `N == id.get()`
- [ ] #3 Test `client_id_sequential` — two back-to-back `ClientId::new()` calls yield IDs that differ by 1
- [ ] #4 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Open `crates/net/src/server/client_id.rs`
2. Add a `#[cfg(test)] mod tests { … }` block at the bottom
3. Import `super::ClientId`
4. Write the three tests listed above
5. Run `cargo nextest run --all-features -p hoy-net client_id` to confirm they pass
6. Run `just coverage` and verify `client_id.rs` reaches 100 % line coverage
7. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
