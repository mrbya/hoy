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
- [`dbstore` module](#dbstore-module)
  * [`DbStore`](#dbstore)
- [`cli` module](#cli-module)
  * [`Hoy`](#hoy)
- [`error` module](#error-module)
  * [`StoreError`](#storeerror)
  * [`HoyError`](#hoyerror)

<!-- tocstop -->

## Lib modules

- `error`: `StoreError`, `HoyError`
- `store`: `RoomName`, `RoomRecord`, `StoredMessage`, `ServerStore`
- `memory`: `InMemoryStore` — in-memory `ServerStore` implementation (always available)
- `dbstore`: `DbStore` — SQLite-backed `ServerStore` implementation
- `cli`: `Hoy` — CLI argument parsing and application entry-point helpers

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
| `RoomName::general()` | Convenience constructor for the `"general"` room (panics only on internal logic errors) |
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

All methods except `storage_slug` are async, expressed as RPIT (`fn method() -> impl Future<Output = …> + Send`) for `Send`-safe use across tokio tasks.

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
    fn storage_slug(&self) -> String;
}
```

| Method | Description |
|---|---|
| `ensure_room(name)` | Create `name` if it does not exist; idempotent |
| `load_rooms()` | Return all persisted rooms |
| `append_message(msg)` | Append a message to a room's history; errors if the room does not exist |
| `load_recent_messages(room, limit)` | Return up to `limit` most recent messages, oldest first; errors if the room does not exist |
| `storage_slug()` | Return a short human-readable label for the store type (used in the server start-up banner) |

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

### `DbStore`

A SQLite-backed `ServerStore` that persists room definitions and message history across server restarts. Uses `sqlx` with a connection pool and applies schema migrations automatically on construction.

```rust
use hoy_core::dbstore::DbStore;
use hoy_core::store::{RoomName, ServerStore};

// Pass None to use the platform data directory (e.g. ~/.local/share/hoy/hoy.db on Linux)
let store = DbStore::new(None).await?;

// Or supply an explicit path to the database file
let store = DbStore::new(Some("/var/lib/hoy/hoy.db".into())).await?;

let general = RoomName::new("general")?;
let messages = store.load_recent_messages(&general, 50).await?;
```

| Method | Description |
|---|---|
| `DbStore::new(path)` | Open (or create) the database at `path`; runs pending migrations. Pass `None` to resolve the path via the platform data directory. |
| `DbStore::close()` | Explicitly close the underlying connection pool. |
| `fetch_room_id(room)` | Look up the internal row ID for a room name; returns `None` if the room does not exist. |

**Storage location** (when `path` is `None`):

| Platform | Default path |
|---|---|
| Linux | `~/.local/share/hoy/hoy.db` |
| macOS | `~/Library/Application Support/com.hoy.hoy/hoy.db` |
| Windows | `%APPDATA%\hoy\hoy\data\hoy.db` |

`DbStore` derives `Debug` and `Clone` (the underlying `SqlitePool` is reference-counted).

---

## `cli` module

Provides CLI argument parsing and application bootstrap helpers, so the binary entry point stays thin.

### `Hoy`

The top-level application handle. Constructed by parsing CLI arguments with `clap` via `Hoy::default()`.

```rust
use hoy_core::cli::Hoy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hoy = Hoy::default(); // parses std::env::args()

    let addr = hoy.resolve_address();

    if hoy.run_server() {
        let store = hoy.construct_store().await?;
        // run_server(addr, store).await?;
    } else {
        let username = hoy.resolve_username()?;
        // run_test_client(addr, username).await?;
    }

    Ok(())
}
```

| Method | Return type | Description |
|---|---|---|
| `Hoy::default()` | `Hoy` | Parse CLI args from `std::env::args()` via `clap` |
| `resolve_address()` | `SocketAddr` | Server address from `--address` / `--port`; defaults to `127.0.0.1:7777` |
| `construct_store()` | `Result<DbStore, StoreError>` | Open (or create) the SQLite store; uses `--db` path or platform default |
| `incognito_store()` | `InMemoryStore` | Return a fresh in-memory store (no persistence) |
| `run_server()` | `bool` | `true` if `-s/--server` flag was passed |
| `resolve_username()` | `Result<String, HoyError>` | Extract `--username`; returns `HoyError::NoUsername` if absent |

**CLI flags**:

| Flag | Short | Default | Description |
|---|---|---|---|
| `--server` | `-s` | `false` | Run in server mode |
| `--port` | `-p` | `7777` | Port to bind or connect to |
| `--address` | `-a` | `127.0.0.1` | Server IPv4 address |
| `--username` | `-u` | — | Client username (required in client mode) |
| `--db` | `-d` | platform default | Path to the SQLite database file |

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

### `HoyError`

| Variant | When |
|---|---|
| `NoUsername` | `Hoy::resolve_username()` called but `--username` was not provided on the CLI |
