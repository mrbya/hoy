---
id: TASK-30
title: Await store calls in main.rs
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
Add .await to all ServerStore call sites in src/main.rs.

Call sites to update (all are inside #[tokio::main] so already in async context):
- store.ensure_room(&room1)? -> store.ensure_room(&room1).await?  (~line 85)
- store.ensure_room(&room2)? -> store.ensure_room(&room2).await?  (~line 86)
- store.append_message(message)? -> store.append_message(message).await?  (~line 88)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All three store call sites in main.rs have .await before ?
- [ ] #2 just check passes
<!-- AC:END -->
