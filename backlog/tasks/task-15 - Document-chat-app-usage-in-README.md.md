---
id: TASK-15
title: Document chat app usage in README.md
status: Done
assignee:
  - '@claude'
created_date: '2026-03-11 19:07'
updated_date: '2026-03-11 19:15'
labels:
  - docs
dependencies:
  - TASK-14
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Expand README.md with user-facing documentation covering how to use the hoy chat application. Currently the README only has crate listing, build commands, and license info. It needs usage documentation so users know how to run the server, connect clients, and use the chat.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 README documents how to start the server (port configuration via CLI args)
- [x] #2 README documents how to connect as a client (username, host/port)
- [x] #3 README documents available chat commands and message flow
- [x] #4 README documents the wire protocol at a high level (for developers/contributors)
- [x] #5 README table of contents is updated (just index)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read main.rs CLI args and test_client.rs to understand all commands and options
2. Read protocol packet definitions to document the wire protocol
3. Write Usage section covering server startup and client connection
4. Write Chat Commands section covering /ping, /disconnect, /quit, /exit, and plain messages
5. Write Protocol section for developers
6. Run just index to update the table of contents
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Usage and Protocol sections to README.md.

Changes:
- Usage section: server startup (default port 7777, -p flag), client connection (-u username, -p port), chat commands table (/ping, /disconnect, /quit, /exit)
- Protocol section: frame format (4-byte big-endian length prefix + JSON), client packets table (Hello, SendMessage, Ping), server packets table (Welcome, ChatMessage, SystemMessage, Error, Pong)
- Fixed "Networkig" typo in Crates list
- Ran just index to regenerate table of contents
<!-- SECTION:FINAL_SUMMARY:END -->
