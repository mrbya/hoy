---
id: TASK-20
title: Add RoomName validation unit tests
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
`RoomName::new` in `crates/core/src/store.rs` has three error branches that are currently
untested: the empty string guard, the length > 64 guard, and the invalid character guard.
The `Display` impl (`fmt::Display for RoomName`) is also uncovered.

Adding a small `#[cfg(test)]` module directly in `store.rs` will cover these 10 missed lines
and lift `hoy-core`'s line coverage from 70 % to ≥ 95 %.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Test `room_name_empty_is_error` — `RoomName::new("")` returns `Err(StoreError::InvalidRoomName(…))`
- [ ] #2 Test `room_name_too_long_is_error` — `RoomName::new` with a 65-character string returns `Err(…)`
- [ ] #3 Test `room_name_invalid_char_is_error` — `RoomName::new("Bad Room!")` (uppercase + space + `!`) returns `Err(…)`, each variant in a separate assert or its own test
- [ ] #4 Test `room_name_valid_succeeds` — `RoomName::new("my-room_01")` returns `Ok(…)`
- [ ] #5 Test `room_name_display` — `format!("{}", RoomName::new("general").unwrap())` equals `"general"`
- [ ] #6 `just test` passes with no failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Open `crates/core/src/store.rs`
2. Add a `#[cfg(test)] mod tests { … }` block at the bottom (or extend one if it already exists)
3. Import `super::RoomName` and `crate::error::StoreError`
4. Write the tests listed in the acceptance criteria
5. Run `cargo nextest run --all-features -p hoy-core` to confirm the new tests pass
6. Run `just coverage` and verify `store.rs` line coverage improved to ≥ 95 %
7. Run `just test` to ensure no regressions
<!-- SECTION:PLAN:END -->
