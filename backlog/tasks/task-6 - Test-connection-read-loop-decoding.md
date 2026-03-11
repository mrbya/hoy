---
id: TASK-6
title: Test connection read loop decoding
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:37'
updated_date: '2026-03-09 23:43'
labels: []
dependencies: []
references:
  - crates/net/src/connection.rs
  - crates/net/src/command.rs
  - crates/protocol/src/codec.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests that feed encoded ClientPacket frames into the connection read loop and assert ServerCommand::Packet emissions and proper disconnect handling.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Read loop decodes a valid frame into ServerCommand::Packet with the expected client_id and packet
- [x] #2 Read loop tolerates partial frames by not emitting a command until complete
- [x] #3 On EOF, connection sends ServerCommand::Disconnected
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Use ConnectionHarness with duplex stream to drive inbound bytes.
2. Add test that writes an encoded ClientPacket frame to the client stream and asserts a ServerCommand::Packet with matching client_id and packet.
3. Add test that writes a partial frame, asserts no Packet command, then writes the remainder and asserts Packet.
4. Add test that closes client stream and asserts ServerCommand::Disconnected.
5. Run cargo test -p hoy-net.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added read-loop tests for complete frame decode, partial-frame buffering, and EOF disconnect using ConnectionHarness.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added connection read-loop unit tests covering frame decoding, partial frame buffering, and disconnect on EOF using in-memory duplex streams.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
