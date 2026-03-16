---
id: TASK-27
title: Make ServerStore trait methods async
status: Done
assignee: []
created_date: '2026-03-16 10:29'
updated_date: '2026-03-16 10:34'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Convert all four ServerStore method signatures in crates/core/src/store.rs from synchronous to async fn.

Methods to update:
- fn ensure_room -> async fn ensure_room
- fn load_rooms -> async fn load_rooms
- fn append_message -> async fn append_message
- fn load_recent_messages -> async fn load_recent_messages

MSRV is 1.85.0 so native async fn in traits is fully supported — no async-trait crate needed.
The trait already carries Send + 'static bounds on the implementor; verify these remain sufficient.
No implementation bodies change in this task — only the trait definition.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All four trait method signatures have async keyword
- [ ] #2 No async-trait dependency added
- [ ] #3 just check passes
<!-- AC:END -->
