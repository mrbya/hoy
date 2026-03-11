# hoy-net
[![crates.io](https://img.shields.io/crates/v/hoy-net.svg)](https://crates.io/crates/hoy-net)
[![docs.rs](https://img.shields.io/docsrs/hoy-net)](https://docs.rs/hoy-net)

Hoy chat app networking logic.

## Lib modules

- `error`: networking errors
- `client`:
    * `command`: client command types
    * `core`: client core event loop and handles
    * `event`: frontend-facing events
    * `session`: client session implementation
    * `state`: client state machine
    * `test_client`: client impl with a tiny stdio frontend to test core and networking
- `server`:
    * `client_id`: tiny strongly-typed client identifier
    * `command`: commands sent from nonnection tasks into the central server state loop
    * `connection`: per-connection read/write task plumbing
    * `core`: TCP listener, accept loop, central state loop and high-level orchestration

## Client design
- one owner of client state
- one command input channel from UI
- one event output channel towards UI
- socket reader/writer tasks hidden behind this boundary

## Server design

- only one task owns chat state
- connection tasks never mutate global state directly
- connection tasks send commands into the server loop
- server loop sends packets back to clients through per-client `mpsc::Sender<ServerPacket>` channel

## Protocol behaviour

1. client connects
2. 1st packet has to be `ClientPacket::Hello { username }`
3. if 1st packet differs:
    - send `ServerPacket::Error`
    - disconnect
4. after successfull hello:
    - assign client to `#general`
    - send `ServerPacket::Welcome`
    - broadcast join message
5. when client sends `SendMessage { text }`:
    - broadcast to all connected clients
6. when client disconnects:
    - broadcast leave message
