# hoy-net
[![crates.io](https://img.shields.io/crates/v/hoy-net.svg)](https://crates.io/crates/hoy-net)
[![docs.rs](https://img.shields.io/docsrs/hoy-net)](https://docs.rs/hoy-net)

Hoy chat app networking logic.

## Lib modules

- `error`: `NetError` and `StateError` types
- `client`:
    * `command`: `ClientCommand` — frontend → client core
    * `core`: client core event loop, `ClientHandle`, `ClientEventStream`
    * `event`: `ClientEvent` — client core → frontend
    * `session`: `SessionHandle`, reader/writer task plumbing
    * `state`: `ClientState` state machine
    * `test_client`: stdio-based test client exercising core and networking
- `server`:
    * `client_id`: strongly-typed `ClientId` (atomic `u64` counter)
    * `command`: `ServerCommand` — connection tasks → central server loop
    * `connection`: TCP accept loop and per-connection read/write tasks
    * `core`: TCP listener, accept loop, central state loop, command dispatch
    * `handlers`: packet handlers and broadcast helpers
    * `state`: `ServerState`, `ClientHandle`, `RoomRuntimeState`

---

## Client design

- One task owns `ClientState` exclusively — no shared mutable state
- One `ClientCommand` channel from the frontend (UI)
- One `ClientEvent` channel toward the frontend
- Reader and writer session tasks are hidden behind `SessionHandle`

### Client state machine

```
Disconnected ──Connect──▶ AwaitingWelcome ──Welcome──▶ Connected
     ▲                          │                          │
     └──────────────────────────┴──Disconnect/Error────────┘
```

| State | Meaning |
|---|---|
| `Disconnected` | No active session |
| `AwaitingWelcome` | TCP connected, `Hello` sent, waiting for `Welcome` |
| `Connected` | Handshake complete; messages, room commands available |

### Client commands (`ClientCommand`)

| Command | Requires state | Effect |
|---|---|---|
| `Connect { server_addr, username }` | Disconnected | TCP connect → send Hello → AwaitingWelcome |
| `Disconnect` | Any | Tear down session → Disconnected |
| `SendMessage { text }` | Connected | Send `SendMessage` packet |
| `Ping` | Connected | Send `Ping` packet |
| `JoinRoom { room }` | Connected | Send `JoinRoom` packet |
| `ListRooms` | Connected | Send `ListRooms` packet |
| `Shutdown` | Any | Tear down session and exit client loop |

### Client events (`ClientEvent`)

| Event | Emitted when |
|---|---|
| `Connecting { server_addr, username }` | Connect command starts |
| `Connected { server_addr, username, room }` | `Welcome` received |
| `Disconnected` | Session ends or error occurs |
| `MessageReceived { from, room, text }` | `ChatMessage` received |
| `SystemMessage { text }` | `SystemMessage` received |
| `RoomJoined { room, messages }` | `RoomJoined` received; `messages` is a `Vec<MessageRecord>` with up to 50 recent messages |
| `RoomList { rooms }` | `RoomList` received |
| `Pong` | `Pong` received |
| `Error { message }` | `Error` packet or local failure |

`MessageRecord` (re-exported from `hoy-protocol`) carries `from: String` and `text: String`.

---

## Server design

- One task owns all mutable chat state (`ServerState`) — serialised, no locks
- Connection tasks never mutate global state directly
- Connection tasks send `ServerCommand`s into the central server loop
- The server loop sends `ServerPacket`s back through per-client `mpsc::Sender<ServerPacket>` channels
- Rooms are persistent (backed by `ServerStore`) and hydrated on startup
- The default room `#general` is always guaranteed to exist

### Server state

`ServerState` tracks:
- **Identified clients** — username, current room, outgoing packet channel
- **Room membership** — `HashSet<ClientId>` per room, always mirroring each client's `current_room`

Clients go through two phases on the server:

| Phase | Map | Can do |
|---|---|---|
| **Pending** | `PendingClients` | Receive `Pong` only |
| **Identified** | `ServerState.clients` | Send messages, join/list rooms |

---

## Protocol behaviour

### Handshake

1. Client connects — assigned a `ClientId`, placed in `PendingClients`
2. First packet must be `Hello { username }`
   - On collision: `Error { "Username … is already in use" }` sent, client remains pending
   - On success:
     - Client moved from pending → identified in `#general`
     - `Welcome { username, room: "general" }` sent to client
     - `SystemMessage: "{username} joined #general"` broadcast to existing room members (sender excluded)
     - `RoomJoined { room: "general", messages: [...] }` sent to client (up to 50 most recent messages from `#general`)
3. `Ping` is answered with `Pong` even before `Hello`

### Messaging

- `SendMessage { text }` → `ChatMessage { from, room, text }` broadcast to **all** members of the sender's current room (including sender)
- Messages are also persisted to the store (non-fatal on storage error)

### Rooms

- `JoinRoom { room }` (name validated; created if new):
  - `SystemMessage: "{username} left #old_room"` broadcast to old room (all members) — skipped if the client is already in the target room
  - `RoomJoined { room, messages }` sent to client; `messages` contains up to 50 most recent messages from the target room, oldest first
  - `SystemMessage: "{username} joined #new_room"` broadcast to new room (sender excluded)
- `ListRooms` → `RoomList { rooms }` sent to client (alphabetically sorted)

### Disconnect

- Client disconnect → removed from state
  - If identified: `SystemMessage: "{username} left #{room}"` broadcast to the client's room
