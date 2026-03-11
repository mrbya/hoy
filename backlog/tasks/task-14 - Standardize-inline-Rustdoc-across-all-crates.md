---
id: TASK-14
title: Standardize inline Rustdoc across all crates
status: Done
assignee:
  - '@claude'
created_date: '2026-03-11 19:07'
updated_date: '2026-03-11 19:12'
labels:
  - docs
  - quality
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Audit and fix all inline documentation to match the project style guide (docs/rustdoc_style.md). Every function (public and private) must use doxygen-style /** */ blocks with the required sections (# Arguments, # Returns, # Errors, # Panics). Single-line /// is reserved for struct fields and enum variants. Macros must also be documented.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All functions in crates/net/src/client/core.rs have /** */ docs with required sections (client_loop, handle_command, handle_command_with_spawner, handle_internal_event, handle_server_packet, emit_event, emit_error, shutdown_state)
- [x] #2 All functions in crates/net/src/client/session.rs have /** */ docs with required sections (spawn_reader_task, spawn_reader_task_io, spawn_writer_task, spawn_writer_task_io, emit_internal_event)
- [x] #3 All functions in crates/net/src/client/test_client.rs have /** */ docs (spawn_input_thread, handle_input_line, print_client_event)
- [x] #4 src/main.rs main() function is documented
- [x] #5 Macros in crates/test/src/macros.rs (assert_err!, async_ok!, assert_matches!) are documented
- [x] #6 Missing type-level docs added for structs: ClientHandle and ClientEventStream in client/core.rs, ClientHandle and ServerState in server/core.rs
- [x] #7 InternalEvent in client/session.rs uses consistent doc style (not //!)
- [x] #8 Shutdown variant in client/command.rs uses /// instead of /** */ (consistent with other enum variants)
- [x] #9 just doc builds without warnings
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Fix client/core.rs: add /** */ docs to 8 undocumented private functions + type-level docs for ClientHandle and ClientEventStream
2. Fix client/session.rs: add /** */ docs to 5 undocumented private functions + fix InternalEvent doc style
3. Fix client/test_client.rs: add /** */ docs to 3 undocumented functions
4. Fix client/command.rs: change Shutdown variant from /** */ to ///
5. Fix src/main.rs: document main()
6. Fix test/macros.rs: document all 3 macros
7. Fix server/core.rs: add type-level docs for ClientHandle and ServerState
8. Run just doc and verify no warnings
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Standardized inline Rustdoc across the codebase to match docs/rustdoc_style.md.

Changes:
- client/core.rs: Fixed emit_event/emit_error argument docs, added missing # Returns to handle_command_with_spawner, added # Arguments to shutdown_state, fixed typos (emmit → emit)
- client/session.rs: Converted spawn_reader_task, spawn_writer_task from /// to /** */ with full sections, added # Returns to spawn_reader_task_io and spawn_writer_task_io, fixed typos (emmited, Ap protocol)
- client/test_client.rs: Added /** */ docs to spawn_input_thread and print_client_event, added # Arguments and # Returns to handle_input_line, added # Returns to format_client_event
- client/command.rs: Changed Shutdown variant from /** */ to /// (consistent with other enum variants)
- src/main.rs: Documented Cli struct, its fields, and main() function
- test/macros.rs: Documented all three macros (assert_err!, async_ok!, assert_matches!)

Verification: just doc, just check, and just test all pass cleanly.
<!-- SECTION:FINAL_SUMMARY:END -->
