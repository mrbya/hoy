# hoy-core
[![crates.io](https://img.shields.io/crates/v/hoy-core.svg)](https://crates.io/crates/hoy-core)
[![docs.rs](https://img.shields.io/docsrs/hoy-core)](https://docs.rs/hoy-core)

Shared domain logic and persistent storage abstractions for the hoy app.

## Index

<!-- toc -->

- [Lib modules](#lib-modules)
- [Features](#features)
- [`store` module](#store-module)
  * [`RoomName`](#roomname)
  * [`RoomRecord`](#roomrecord)
  * [`StoredMessage`](#storedmessage)
  * [`ServerStore` trait](#serverstore-trait)
- [`memory` module](#memory-module)
  * [`InMemoryStore`](#inmemorystore)
- [`dbstore` module](#dbstore-module)
  * [`DbStore`](#dbstore)
- [`error` module](#error-module)
  * [`StoreError`](#storeerror)

<!-- tocstop -->

## Lib modules

- `error`: `StoreError`
- `store`: `RoomName`, `RoomRecord`, `StoredMessage`, `ServerStore`
- `memory`: `InMemoryStore` — in-memory `ServerStore` implementation (always available)
- `dbstore`: `DbStore` — SQLite-backed `ServerStore` implementation (requires `dbstore` feature)

---

## Features

| Feature | Description |
|---|---|
| `dbstore` | Enables `DbStore`, a SQLite-backed `ServerStore`. Pulls in `sqlx` and `directories`. |

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
| `RoomName::general()` | Convenience constructor for the `"general"` room |
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

All methods are async and return `impl Future + Send`, making the trait safe to use across tokio task boundaries.

```rust
pub trait ServerStore: Send + 'static {
    fn ensure_room(&mut self, name: &RoomName)
        -> impl Future<Output = Result<(), StoreError>> + Send;
    fn load_rooms(&self)
        -> impl Future<Output = Result<Vec<RoomRecord>, StoreError>> + Send;
    fn append_message(&mut self, msg: StoredMessage)
        -> impl Future<Output = Result<(), StoreError>> + Send;
    fn load_recent_messages(&self, room: &RoomName, limit: usize)
        -> impl Future<Output = Result<Vec<StoredMessage>, StoreError>> + Send;
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

A `HashMap`-backed `ServerStore` for use in tests and lightweight scenarios. All data is held in process memory and is not persisted across restarts.

```rust
use hoy_core::memory::InMemoryStore;
use hoy_core::store::{RoomName, ServerStore};

let mut store = InMemoryStore::new();
let general = RoomName::new("general")?;

store.ensure_room(&general).await?;

let rooms = store.load_rooms().await?;
assert_eq!(rooms.len(), 1);
```

`load_rooms` returns rooms sorted alphabetically by name. `load_recent_messages` returns the last `limit` messages (tail of the chronological history), oldest first.

---

## `dbstore` module

Requires the `dbstore` feature flag.

### `DbStore`

A SQLite-backed `ServerStore` that persists room definitions and message history across server restarts. Uses `sqlx` with a connection pool and applies schema migrations automatically on construction.

```rust
use hoy_core::dbstore::DbStore;
use hoy_core::store::{RoomName, ServerStore};

// Pass None to use the platform data directory (e.g. ~/.local/share/hoy/hoy.db)
let store = DbStore::new(None).await?;

// Or supply an explicit directory path
let store = DbStore::new(Some("/var/lib/hoy".into())).await?;

let general = RoomName::new("general")?;
let messages = store.load_recent_messages(&general, 50).await?;
```

| Method | Description |
|---|---|
| `DbStore::new(path)` | Open (or create) the database at `path/hoy.db`; runs pending migrations. Pass `None` to resolve the path via the platform data directory. |
| `fetch_room_id(room)` | Look up the internal row ID for a room name; returns `None` if the room does not exist. |

**Storage location** (when `path` is `None`):

| Platform | Default path |
|---|---|
| Linux | `~/.local/share/hoy/hoy.db` |
| macOS | `~/Library/Application Support/com.hoy.hoy/hoy.db` |
| Windows | `%APPDATA%\hoy\hoy\data\hoy.db` |

`DbStore` derives `Debug` and `Clone` (the underlying `SqlitePool` is reference-counted).

---

## `error` module

### `StoreError`

| Variant | When |
|---|---|
| `InvalidRoomName(String)` | `RoomName::new` received an empty, too-long, or character-invalid string |
| `RoomNotFound(RoomName)` | `append_message` or `load_recent_messages` references a room that does not exist |
| `Internal(String)` | Generic internal store failure (e.g. database query error) |
| `NoDataDirectory` | The platform data directory could not be resolved (`DbStore::new(None)`) |
| `Io(std::io::Error)` | I/O error while creating or accessing the storage directory |
