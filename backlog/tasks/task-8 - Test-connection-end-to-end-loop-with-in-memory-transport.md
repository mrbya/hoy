---
id: TASK-8
title: Test connection end-to-end loop with in-memory transport
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:37'
updated_date: '2026-03-09 23:47'
labels: []
dependencies: []
references:
  - crates/net/src/connection.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add a combined unit test that drives handle_connection with in-memory duplex stream to validate read and write plumbing together.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Using a tokio duplex stream, handle_connection processes an incoming ClientPacket and emits ServerCommand::Packet
- [x] #2 When the server sends a ServerPacket via client channel, the stream contains the encoded frame
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Use ConnectionHarness with duplex stream to exercise both read and write paths in a single test.
2. Write an encoded ClientPacket frame to the client stream and assert ServerCommand::Packet emission.
3. Use the Connected command’s tx to send a ServerPacket and verify the client stream receives the encoded frame.
4. Run cargo test -p hoy-net.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added an end-to-end duplex test that writes a ClientPacket and asserts ServerCommand::Packet, then sends a ServerPacket and verifies the encoded frame on the client stream.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added an end-to-end connection loop unit test using in-memory duplex streams to validate both read and write plumbing together.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
