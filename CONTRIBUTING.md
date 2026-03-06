# Contributing

Thanks for your interest in contributing!

## Getting started
- Install `just`: `cargo install just`
- Install toolchain and hooks: `just init`
- See available workflows: `just list`

## Development workflow
- Format: `just fmt`
- Lint: `just check`
- Tests: `just test`
- Full check: `just pre-commit`

## Code style
- Follow Rust standard naming (`snake_case` for functions/modules, `CamelCase` for types/traits).
- Keep modules focused; prefer explicit `mod` boundaries when adding new components.
- Rust formatting is enforced by `rustfmt` (see `rustfmt.toml`).

## Tests
- Integration tests live in `tests/`.
- Benchmarks live in `benches/` and run with `just benchmark`.
- Coverage reports: `just coverage`.

## Docs and changelog
- If a change affects CLI behavior or config formats, update `README.md`.
- Keep docs concise and consistent with `docs/rustdoc_style.md`.

## Submitting changes
1. Create a focused branch per change.
2. Keep commits small and descriptive (plain-English summaries).
3. Open a PR with:
   - A concise summary
   - Tests run (e.g., `just test`)
   - Notable behavior or docs changes

## Repor layout
- Protected branches: `master`, `dev`.
- we're branching out of dev.
- `master` contains stable releases.

## Reporting issues
- Include OS, Rust version, and reproduction steps.
- Provide logs or minimal examples when possible.

## License
Any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed under the Apache-2.0 and
MIT license, without any additional terms or conditions.

[LICENSE-APACHE]: ./LICENSE-APACHE
[LICENSE-MIT]: ./LICENSE-MIT
