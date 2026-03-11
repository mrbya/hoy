---
id: TASK-4
title: Test server disconnect behavior
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:23'
updated_date: '2026-03-09 23:35'
labels: []
dependencies: []
references:
  - crates/net/src/server.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests for disconnect handling and related helpers in the server loop.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Disconnected client with a username triggers SystemMessage left broadcast
- [x] #2 Disconnected client without a username triggers no broadcast
- [x] #3 username_of and username_exists behave correctly for initialized and uninitialized clients
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add disconnect tests in server.rs using TestHarness to observe broadcasts.
2. Verify SystemMessage left is broadcast when a named client disconnects and no broadcast when unnamed.
3. Add unit tests for username_of and username_exists for initialized and uninitialized clients.
4. Run cargo test -p hoy-net.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added disconnect behavior tests (broadcast on named disconnect, none on unnamed) and username helper checks.
Added TestHarness::try_recv_now to assert absence of broadcast.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added server loop unit tests for disconnect handling and username helper behavior, including broadcast on named disconnect and no broadcast on unnamed.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
