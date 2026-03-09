# Repository Guidelines

## Project Structure & Module Organization
- Workspace root is the `hoy` binary in `src/` (`src/main.rs`, `src/lib.rs`).
- Core crates live under `crates/`: `core/` (domain logic), `net/` (client/server networking), `protocol/` (wire packets/codec), `tui/` (terminal UI).
- Tests are in `tests/` (integration/e2e) and benchmarks in `benches/`.
- Documentation lives in `docs/`, especially `docs/rustdoc_style.md` for Rustdoc conventions.

## Build, Test, and Development Commands
This repo uses `just` as the primary task runner (install with `cargo install just`).
- `just init`: install toolchain, hooks, and required tooling.
- `just run`: build and run the `hoy` binary.
- `just build`: release build for the workspace.
- `just test`: run the full test suite via `cargo nextest`.
- `just check`: run Clippy across all targets/features.
- `just fmt`: apply `rustfmt` (nightly).
- `just pre-commit`: run formatting, linting, tests, docs, and other checks.

## Coding Style & Naming Conventions
- Follow Rust standard naming: `snake_case` for functions/modules, `CamelCase` for types/traits.
- Formatting is enforced by `rustfmt` with `rustfmt.toml` (module-level import granularity, grouped imports).
- Document functions with doxygen-style `/** ... */` blocks per `docs/rustdoc_style.md`.

## Testing Guidelines
- Integration tests live in `tests/` (e.g., `tests/integration_tests.rs`).
- Benchmarks live in `benches/` and run with `just benchmark`.
- Coverage is available via `just coverage` (llvm-cov + nextest).

## Commit & Pull Request Guidelines
- Commit messages are short, plain-English summaries (e.g., “Added try-decode”, “Fixed clippy setups”).
- Branch from `dev`; `master` and `dev` are protected.
- PRs should include: a concise summary, tests run (e.g., `just test`), and any behavior/docs changes.
- If a change affects CLI behavior or config formats, update `README.md` and keep docs aligned with `docs/rustdoc_style.md`.

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
