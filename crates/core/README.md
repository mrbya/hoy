# hoy-core
[![crates.io](https://img.shields.io/crates/v/hoy-core.svg)](https://crates.io/crates/hoy-core)
[![docs.rs](https://img.shields.io/docsrs/hoy-core)](https://docs.rs/hoy-core)

Shared domain logic and persistent storage abstractions for the hoy app.

## Index

<!-- toc -->

- [Lib modules](#lib-modules)
- [`store` module](#store-module)
  * [`RoomName`](#roomname)
  * [`RoomRecord`](#roomrecord)
  * [`StoredMessage`](#storedmessage)
  * [`ServerStore` trait](#serverstore-trait)
- [`memory` module](#memory-module)
  * [`InMemoryStore`](#inmemorystore)
- [`error` module](#error-module)
  * [`StoreError`](#storeerror)

<!-- tocstop -->

## Lib modules

- `error`: `StoreError`
- `store`: `RoomName`, `RoomRecord`, `StoredMessage`, `ServerStore`
- `memory`: `InMemoryStore` — in-memory `ServerStore` implementation

---

## `store` module

### `RoomName`

A validated, owned room name. Enforces:

- **Allowed characters**: lowercase ASCII letters (`a–z`), ASCII digits (`0–9`), `-`, `_`
- **Length**: 1–64 characters (inclusive)

```rust
let name = RoomName::new("my-room_01")?;  // Ok
let _    = RoomName::new("");             // Err — empty
let _    = RoomName::new("Bad Room!");    // Err — uppercase, space, '!'
```

| Item | Description |
|---|---|
| `RoomName::GENERAL` | `"general"` — the default room every client joins on connect |
| `RoomName::new(s)` | Construct and validate; returns `Err(StoreError::InvalidRoomName)` on failure |
| `as_str()` | Borrow the inner string slice |
| `into_string()` | Consume into the inner `String` |
| `Display` | Formats as the plain room name string |

`RoomName` derives `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`.

### `RoomRecord`

A persisted room entry returned by `ServerStore::load_rooms`:

| Field | Type | Description |
|---|---|---|
| `name` | `RoomName` | Canonical room name |

### `StoredMessage`

A single persisted chat message:

| Field | Type | Description |
|---|---|---|
| `room` | `RoomName` | Room the message was sent in |
| `from` | `String` | Sender username |
| `text` | `String` | Message body |

### `ServerStore` trait

The persistence interface for durable server data — room definitions and message history. Live connection state (connected clients, active memberships, writer channels) is **not** managed here; that belongs to the network layer.

```rust
pub trait ServerStore: Send + 'static {
    fn ensure_room(&mut self, name: &RoomName) -> Result<(), StoreError>;
    fn load_rooms(&self) -> Result<Vec<RoomRecord>, StoreError>;
    fn append_message(&mut self, msg: StoredMessage) -> Result<(), StoreError>;
    fn load_recent_messages(&self, room: &RoomName, limit: usize)
        -> Result<Vec<StoredMessage>, StoreError>;
}
```

| Method | Description |
|---|---|
| `ensure_room(name)` | Create `name` if it does not exist; idempotent |
| `load_rooms()` | Return all persisted rooms |
| `append_message(msg)` | Append a message to a room's history; errors if the room does not exist |
| `load_recent_messages(room, limit)` | Return up to `limit` most recent messages, oldest first; errors if the room does not exist |

---

## `memory` module

### `InMemoryStore`

A `HashMap`-backed `ServerStore` implementation for use in tests and as the default runtime store. All data is held in process memory and is not persisted across restarts.

```rust
use hoy_core::memory::InMemoryStore;
use hoy_core::store::{RoomName, ServerStore};

let mut store = InMemoryStore::new();
let general = RoomName::new("general")?;

store.ensure_room(&general)?;

let rooms = store.load_rooms()?;
assert_eq!(rooms.len(), 1);
```

`load_rooms` returns rooms sorted alphabetically by name. `load_recent_messages` returns the last `limit` messages (tail of the chronological history), oldest first.

---

## `error` module

### `StoreError`

| Variant | When |
|---|---|
| `InvalidRoomName(String)` | `RoomName::new` received an empty, too-long, or character-invalid string |
| `RoomNotFound(RoomName)` | `append_message` or `load_recent_messages` references a room that does not exist |
| `Internal(String)` | Generic internal store failure |
