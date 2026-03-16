---
id: TASK-28
title: Update InMemoryStore impl to async fn
status: Done
assignee: []
created_date: '2026-03-16 10:29'
updated_date: '2026-03-16 10:36'
labels: []
dependencies:
  - TASK-27
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add the async keyword to all four ServerStore method implementations on InMemoryStore in crates/core/src/memory.rs.

Methods to update:
- fn ensure_room -> async fn ensure_room
- fn load_rooms -> async fn load_rooms
- fn append_message -> async fn append_message
- fn load_recent_messages -> async fn load_recent_messages

The method bodies are purely synchronous (HashMap operations), so no awaiting is needed inside them.

Also audit the test module in memory.rs: any direct calls to these methods from sync test functions will need to be moved into async contexts (e.g. wrapped in async_ok! macro from hoy-test).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All four impl method signatures have async keyword
- [ ] #2 Method bodies unchanged (no .await needed inside them)
- [ ] #3 Test module updated — all store calls in tests are in async context
- [ ] #4 just check passes
<!-- AC:END -->
