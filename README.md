# Hoy! - TUI real-time messaging app
[![crates.io](https://img.shields.io/crates/v/hoy.svg)](https://crates.io/crates/hoy)
[![docs.rs](https://img.shields.io/docsrs/hoy)](https://docs.rs/hoy)
[![pre-commit](https://img.shields.io/badge/pre--commit-enabled-brightgreen?logo=pre-commit&logoColor=white)](https://github.com/pre-commit/pre-commit)
[![pipeline status](https://gitlab.com/byacrates/hoy/badges/master/pipeline.svg)](https://gitlab.com/byacrates/hoy/-/commits/master)
[![Coverage Status](https://coveralls.io/repos/gitlab/byacrates/hoy/badge.svg?branch=master)](https://coveralls.io/gitlab/byacrates/hoy?branch=master)

A TUI real time messaging app inspired by accord.

## Index

<!-- toc -->

- [Installation](#installation)
- [Usage](#usage)
  * [Starting the server](#starting-the-server)
  * [Connecting a client](#connecting-a-client)
  * [Chat commands](#chat-commands)
- [Crates](#crates)
- [Protocol](#protocol)
  * [Frame format](#frame-format)
  * [Client packets and server packets](#client-packets-and-server-packets)
- [Similar projects](#similar-projects)
- [License](#license)
- [Contribution](#contribution)
- [Development](#development)

<!-- tocstop -->

## Installation

Install using cargo:
```bash
cargo install hoy
```

## Usage

Both the server and client currently share the same binary `hoy`:
```bash
Usage: hoy [OPTIONS]

Options:
  -s, --server               Run in server mode instead of client mode
  -p, --port <PORT>          Port to bind the server or connect the client to [default: 7777]
  -a, --address <IPV4>       Server address to connect to. [default: localhost]
  -u, --username <USERNAME>  Client username (required in client mode)
  -h, --help                 Print help
  -V, --version              Print version
```

### Starting the server

Run in server mode with the `-s` flag. The default port is `7777`.

```bash
# Start on default port 7777
hoy -s

# Start on a custom port
hoy -s -p 9000
```

### Connecting a client

Connect as a client by providing a username with `-u`. The client connects to
`127.0.0.1` on the specified port (default `7777`).

```bash
# Connect with username "alice" to default port
hoy -u alice

# Connect to a custom port
hoy -u alice -p 9000
```

On connection, the server places the client in `#general` and broadcasts a join
message to all connected clients.

### Chat commands

Once connected, type in the terminal. Plain text sends a chat message. Special
commands begin with `/`:

| Command       | Action                                      |
|---------------|---------------------------------------------|
| `/list`       | Requests a list of available rooms          |
| `/room <room name>` | Requests joining/creating a room      |
| `/ping`       | Send a heartbeat ping; server replies Pong. |
| `/disconnect` | Disconnect from the server without exiting. |
| `/quit`       | Disconnect and exit the client.             |
| `/exit`       | Alias for `/quit`.                          |

Any other input is sent as a chat message and broadcast to all clients in the
room.

Valid room name:
- Allowed characters: lowercase ASCII letters, Ascii digits, `_`, `-`
- Length: 1-64 characters.

## Crates

1. `hoy-core` - Shared domain logic
2. `hoy-net` - Networking layer
3. `hoy-protocol` - Wire level protocol defining packets and codec
4. `hoy-tui` - App TUI

## Protocol

The wire protocol is useful for contributors or anyone building alternative
clients.

### Frame format

Each packet is encoded as a length-prefixed frame:

```
[4 bytes: big-endian u32 payload length][N bytes: JSON payload]
```

The JSON payload is a serialized `ClientPacket` or `ServerPacket` enum value.
There is no framing delimiter; frames are concatenated directly in the TCP
stream.

### Client packets and server packets

See [hoy-protocol readme](crates/protocol/README.md).

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
