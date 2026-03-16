---
id: TASK-29
title: Await store calls in hoy-net server crate
status: Done
assignee: []
created_date: '2026-03-16 10:29'
updated_date: '2026-03-16 10:38'
labels: []
dependencies:
  - TASK-27
  - TASK-28
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add .await to all ServerStore call sites in crates/net/src/server/.

crates/net/src/server/handlers.rs (production code):
- store.ensure_room(&target) -> store.ensure_room(&target).await  (handle_join_room)
- store.append_message(..) -> store.append_message(..).await  (handle_send_message)
- store.load_recent_messages(..) -> store.load_recent_messages(..).await  (handle_join_room)

crates/net/src/server/core.rs (production code):
- store.load_rooms() -> store.load_rooms().await  (server startup ~line 128)
- store.ensure_room(&default_room) -> store.ensure_room(&default_room).await  (server startup ~line 138)

crates/net/src/server/core.rs (StoreStub test impl):
- Add async to all four ServerStore method signatures on StoreStub (~lines 281-305).
  Bodies remain synchronous.

crates/net/src/server/core.rs (TestHarness):
- store.ensure_room(&general) -> store.ensure_room(&general).await (~line 184).
  TestHarness::new() is currently sync; it may need to become async fn new() with call sites updated, or the ensure_room call restructured.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All three store calls in handlers.rs have .await
- [ ] #2 Both store calls in server startup (core.rs) have .await
- [ ] #3 StoreStub ServerStore impl methods have async keyword
- [ ] #4 TestHarness setup correctly awaits ensure_room
- [ ] #5 just check and just test both pass
<!-- AC:END -->
