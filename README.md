# Hoy! - TUI real-time messaging app
[![crates.io](https://img.shields.io/crates/v/hoy.svg)](https://crates.io/crates/hoy)
[![docs.rs](https://img.shields.io/docsrs/hoy)](https://docs.rs/hoy)
[![pre-commit](https://img.shields.io/badge/pre--commit-enabled-brightgreen?logo=pre-commit&logoColor=white)](https://github.com/pre-commit/pre-commit)

A TUI real time messaging app inspired by accord.

## Index

<!-- toc -->

- [Usage](#usage)
  * [Starting the server](#starting-the-server)
  * [Connecting a client](#connecting-a-client)
  * [Chat commands](#chat-commands)
- [Protocol](#protocol)
  * [Frame format](#frame-format)
  * [Client packets](#client-packets)
  * [Server packets](#server-packets)
- [Crates](#crates)
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
| `/ping`       | Send a heartbeat ping; server replies Pong. |
| `/disconnect` | Disconnect from the server without exiting. |
| `/quit`       | Disconnect and exit the client.             |
| `/exit`       | Alias for `/quit`.                          |

Any other input is sent as a chat message and broadcast to all clients in the
room.

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

### Client packets

Packets sent from client to server:

| Variant       | Fields                   | When sent                          |
|---------------|--------------------------|------------------------------------|
| `Hello`       | `username: String`       | Immediately after TCP connect.     |
| `SendMessage` | `text: String`           | User sends a chat message.         |
| `Ping`        | —                        | User runs `/ping`.                 |

### Server packets

Packets sent from server to client:

| Variant         | Fields                               | When sent                                      |
|-----------------|--------------------------------------|------------------------------------------------|
| `Welcome`       | `username: String`, `room: String`   | Handshake accepted; confirms username and room.|
| `ChatMessage`   | `from: String`, `room: String`, `text: String` | A client sent a message; broadcast to all.  |
| `SystemMessage` | `text: String`                       | Join/leave notifications.                      |
| `Error`         | `message: String`                    | Protocol or state error (e.g. duplicate name). |
| `Pong`          | —                                    | Response to a `Ping`.                          |

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
