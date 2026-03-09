# Hoy! - TUI real-time messaging app
[![crates.io](https://img.shields.io/crates/v/hoy.svg)](https://crates.io/crates/hoy)
[![docs.rs](https://img.shields.io/docsrs/hoy)](https://docs.rs/hoy)
[![pre-commit](https://img.shields.io/badge/pre--commit-enabled-brightgreen?logo=pre-commit&logoColor=white)](https://github.com/pre-commit/pre-commit)

A TUI real time messaging app inspired by accord.

> 🔔 **Note:** This is just an initial pre-release to occupy the crate name.

## Index

<!-- toc -->

- [Similar projects](#similar-projects)
- [License](#license)
- [Contribution](#contribution)
- [Development](#development)

<!-- tocstop -->

## Crates

1. `hoy-core` - Shared domain logic
2. `hoy-net` - Networkig layer
3. `hoy-protocol` - Wire level protocol defining packets and codec
4. `hoy-tui` - App TUI

## Similar projects
- [accord](https://github.com/LoipesMas/accord)

## License

This project is licensed under either of:
* Apache License, Version 2.0, ([LICENSE-APACHE] or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT] or http://opensource.org/licenses/MIT)

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed under the Apache-2.0 and
MIT license, without any additional terms or conditions.

[LICENSE-APACHE]: ./LICENSE-APACHE
[LICENSE-MIT]: ./LICENSE-MIT

## Development

See [contribution guidelines](CONTRIBUTING.md).

TLDR:

Requires `just` to bootstrap all tools and configuration
```bash
cargo install just
just init # setup repo, install hooks and all required tools
```

To run:
```bash
just run
```

To test:
```bash
just test
```

Before committing work:
```bash
just pre-commit
```

To see all available commands:
```bash
just list
```
