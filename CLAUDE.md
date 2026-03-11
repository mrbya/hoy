# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Hoy** is a TUI real-time messaging app written in Rust (Edition 2024, MSRV 1.85.0), organized as a Cargo workspace. The workspace root binary (`src/`) ties together four core crates under `crates/`.

## Commands

This repo uses `just` as the primary task runner. Install it with `cargo install just`, then run `just init` once to set up toolchain, hooks, and required tooling.

| Command | Purpose |
|---|---|
| `just run` | Build and run the binary |
| `just test` | Run full test suite via `cargo nextest` |
| `just check` | Run Clippy on all targets/features |
| `just fmt` | Apply `rustfmt` (nightly) |
| `just doc` | Generate rustdoc (add `-- --open` to open in browser) |
| `just coverage` | Code coverage with `llvm-cov` |
| `just benchmark` | Run criterion benchmarks |
| `just pre-commit` | Full pre-commit check (fmt, check -D warnings, test, doc, thorough-check, index) |
| `just ci` | CI pipeline (non-interactive, no codebase modifications) |
| `just thorough-check` | Check unused deps (`cargo udeps`) + audit vulnerabilities |

To run a single test: `cargo nextest run --all-features --workspace <test-name>`

## Architecture

### Crate Dependency Graph

```
src/main.rs (hoy binary)
├── hoy-net   (client + server networking)
├── hoy-tui   (terminal UI — skeleton)
└── hoy-core  (domain types — minimal)

hoy-net
├── hoy-protocol  (wire protocol)
└── hoy-core

hoy-protocol
└── hoy-core

hoy-test  (shared test macros — dev only)
```

### Crates

- **`hoy-core`** (`crates/core/`) — Domain types (User, Room). Currently minimal/placeholder.
- **`hoy-net`** (`crates/net/`) — Client and server implementations.
  - `client/`: State machine (`ClientState`: Disconnected → AwaitingWelcome → Connected), `ClientHandle`, `SessionHandle` (separate reader/writer tokio tasks), `ClientCommand`/`ClientEvent` for control and frontend events.
  - `server/`: Event loop, per-connection handler, broadcast logic, `ClientId`.
- **`hoy-protocol`** (`crates/protocol/`) — Wire protocol: length-prefixed frames (4-byte big-endian u32 + JSON payload). `ClientPacket` (Hello, SendMessage, Ping) and `ServerPacket` (Welcome, ChatMessage, SystemMessage, Error, Pong). `FrameBuffer` handles streaming/incomplete frames.
- **`hoy-tui`** (`crates/tui/`) — Terminal UI using ratatui. Currently skeleton (app, event, ui modules).
- **`hoy-test`** (`crates/test/`) — Shared test macros: `async_ok!` (async with 200ms timeout), `assert_err!`, `assert_matches!`.

### Key Patterns

- **Actor model**: Client/server use tokio tasks communicating via `mpsc` channels.
- **State machine**: `ClientState` enum with explicit state transitions.
- **Streaming protocol**: `FrameBuffer` handles incomplete/multi-frame TCP reads incrementally.

## Code Style

- Document with `/** ... */` doxygen-style blocks per `docs/rustdoc_style.md`.
- Imports: module-level granularity, grouped (see `rustfmt.toml`).
- Linting is strict: wildcard imports denied, `Rc<Mutex<_>>` denied. Run `just check -- -D warnings` before committing.

## Testing

- Integration tests in `tests/integration_tests.rs` — spawn real servers and TCP clients.
- Unit tests are inline in source modules.
- `hoy-test` macros are available for all crates via dev-dependency.

## Branching & Commits

- Branch from `dev`; `master` and `dev` are protected.
- Commit messages are short, plain-English summaries (e.g., "Added try-decode", "Fixed clippy setups").
- PRs include: concise summary, tests run, and behavior/docs changes.


<!-- BACKLOG.MD MCP GUIDELINES START -->

<CRITICAL_INSTRUCTION>

## BACKLOG WORKFLOW INSTRUCTIONS

This project uses Backlog.md MCP for all task and project management activities.

**CRITICAL GUIDANCE**

- If your client supports MCP resources, read `backlog://workflow/overview` to understand when and how to use Backlog for this project.
- If your client only supports tools or the above request fails, call `backlog.get_workflow_overview()` tool to load the tool-oriented overview (it lists the matching guide tools).

- **First time working here?** Read the overview resource IMMEDIATELY to learn the workflow
- **Already familiar?** You should have the overview cached ("## Backlog.md Overview (MCP)")
- **When to read it**: BEFORE creating tasks, or when you're unsure whether to track work

These guides cover:
- Decision framework for when to create tasks
- Search-first workflow to avoid duplicates
- Links to detailed guides for task creation, execution, and finalization
- MCP tools reference

You MUST read the overview resource to understand the complete workflow. The information is NOT summarized here.

</CRITICAL_INSTRUCTION>

<!-- BACKLOG.MD MCP GUIDELINES END -->
