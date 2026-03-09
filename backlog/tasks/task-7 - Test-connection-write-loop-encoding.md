---
id: TASK-7
title: Test connection write loop encoding
status: Done
assignee:
  - '@codex'
created_date: '2026-03-09 23:37'
updated_date: '2026-03-09 23:45'
labels: []
dependencies: []
references:
  - crates/net/src/connection.rs
  - crates/protocol/src/codec.rs
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add unit tests that drive the connection writer by sending ServerPacket values and asserting encoded frames are written to the stream.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Writer sends encoded ServerPacket frames to the output stream
- [x] #2 Writer task exits cleanly when client channel closes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Use ConnectionHarness to capture the Connected command and extract the client tx channel.
2. Send a ServerPacket via the tx and read bytes from the client stream, then decode with FrameBuffer or decode_frame to assert round-trip.
3. Drop the tx to close the writer channel and assert the handle_connection task completes without error.
4. Run cargo test -p hoy-net.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added write-loop tests to verify encoded ServerPacket frames are written to the stream and that the writer task exits cleanly when the channel closes.
Ran: cargo test -p hoy-net
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added connection write-loop unit tests that validate frame encoding to the output stream and clean shutdown on channel close using in-memory duplex streams.

Tests:
- cargo test -p hoy-net
<!-- SECTION:FINAL_SUMMARY:END -->
