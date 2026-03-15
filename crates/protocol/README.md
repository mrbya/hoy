# hoy-protocol
[![crates.io](https://img.shields.io/crates/v/hoy-protocol.svg)](https://crates.io/crates/hoy-protocol)
[![docs.rs](https://img.shields.io/docsrs/hoy-protocol)](https://docs.rs/hoy-protocol)

Wire-level protocol defining packets and codec of the hoy app.

## Lib modules

- `codec`: `encode_frame`, `decode_frame`, `try_decode_frame`
- `error`: `ProtocolError`
- `frame_buffer`: `FrameBuffer` — streaming byte buffer with incremental decoding
- `packet`: `ClientPacket`, `ServerPacket`

---

## Frame format

Every message is a **length-prefixed frame**:

```
┌──────────────────────┬─────────────────────────────────┐
│  header  (4 bytes)   │  payload  (header value bytes)  │
│  u32, big-endian     │  UTF-8 JSON                     │
└──────────────────────┴─────────────────────────────────┘
```

- **Header** — the payload length encoded as a `u32` in big-endian byte order.
- **Payload** — the JSON-serialised packet (`serde_json`), length in bytes matching the header exactly.
- **Total frame size** — `4 + payload_len` bytes.

There is no maximum payload size enforced beyond the `u32` ceiling (~4 GB).

---

## Packets

### `ClientPacket` — client → server

| Variant | Fields | Description |
|---|---|---|
| `Hello` | `username: String` | Opening handshake; must be the first packet sent |
| `SendMessage` | `text: String` | Broadcast a chat message to the current room |
| `JoinRoom` | `room: String` | Request to join (or create) a room |
| `ListRooms` | — | Request a list of all known rooms |
| `Ping` | — | Heartbeat; answered even before `Hello` |

### `ServerPacket` — server → client

| Variant | Fields | Description |
|---|---|---|
| `Welcome` | `username: String`, `room: String` | Handshake accepted; reports the assigned username and initial room |
| `ChatMessage` | `from: String`, `room: String`, `text: String` | A chat message broadcast to room members |
| `SystemMessage` | `text: String` | Server-generated event text (joins, leaves, etc.) |
| `RoomJoined` | `room: String` | Confirms a successful room change |
| `RoomList` | `rooms: Vec<String>` | Response to `ListRooms`; rooms are alphabetically sorted |
| `Error` | `message: String` | Describes a protocol or application error |
| `Pong` | — | Response to `Ping` |

### JSON wire shapes

Serde serialises unit variants as plain JSON strings and struct variants as single-key objects:

```json
// ClientPacket
{"Hello":       {"username": "alice"}}
{"SendMessage": {"text": "hello all"}}
{"JoinRoom":    {"room": "rust"}}
"ListRooms"
"Ping"

// ServerPacket
{"Welcome":       {"username": "alice", "room": "general"}}
{"ChatMessage":   {"from": "alice", "room": "general", "text": "hello all"}}
{"SystemMessage": {"text": "bob joined #general"}}
{"RoomJoined":    {"room": "rust"}}
{"RoomList":      {"rooms": ["general", "rust"]}}
{"Error":         {"message": "Username \"bob\" is already in use"}}
"Pong"
```

---

## Codec

### `encode_frame(value) -> Result<Vec<u8>, ProtocolError>`

Serialises `value` to JSON, prepends the 4-byte big-endian length header, and returns the complete frame as a `Vec<u8>`.

```
encode_frame(&ClientPacket::Ping)
  → serde_json::to_vec  →  b"\"Ping\""          (6 bytes)
  → header              →  [0, 0, 0, 6]
  → frame               →  [0, 0, 0, 6, '"', 'P', 'i', 'n', 'g', '"']
```

### `decode_frame<T>(frame) -> Result<T, ProtocolError>`

Decodes a buffer that is known to contain at least one complete frame. Returns an error if the buffer is too short (`TruncatedFrame`). Trailing bytes after the frame are silently ignored.

### `try_decode_frame<T>(buffer) -> Result<Option<(T, usize)>, ProtocolError>`

Non-blocking streaming decoder. Returns:

| Result | Meaning |
|---|---|
| `Ok(None)` | Buffer too short — header or payload incomplete; no bytes consumed |
| `Ok(Some((value, consumed)))` | Frame decoded; `consumed` is the number of bytes used (`4 + payload_len`) |
| `Err(e)` | Header valid but payload is malformed JSON, or arithmetic overflow |

The `consumed` count lets the caller advance its read position without any internal mutation.

---

## FrameBuffer

`FrameBuffer` wraps a `Vec<u8>` and provides incremental decoding over a TCP stream:

```rust
let mut buf = FrameBuffer::with_capacity(4096);

// inside a read loop:
buf.append(&tcp_chunk)?;

while let Some(packet) = buf.try_decode::<ServerPacket>()? {
    handle(packet);
}
```

### Methods

| Method | Description |
|---|---|
| `new()` | Create an empty buffer (no heap allocation) |
| `with_capacity(n)` | Preallocate `n` bytes |
| `append(chunk)` | Extend the buffer with new bytes from the wire |
| `try_decode::<T>()` | Attempt to decode one frame; on success the consumed bytes are removed from the front of the buffer |
| `len()` / `is_empty()` | Current byte count |
| `clear()` | Discard all buffered bytes |
| `as_slice()` | Read-only view of the raw bytes |

`try_decode` mirrors `try_decode_frame` semantics — `Ok(None)` means more data is needed, not an error. Each call decodes at most one frame; loop until `None` to drain all available frames.

---

## Error types

`ProtocolError` variants:

| Variant | When |
|---|---|
| `FrameTooLarge { size }` | Serialised payload exceeds `u32::MAX` |
| `TruncatedFrame` | `decode_frame` called on an incomplete buffer |
| `FrameLengthOutOfRange { length }` | Decoded `u32` header does not fit in `usize` (32-bit platforms) |
| `CapacityOverflow` | `4 + payload_len` overflows `usize` |
| `InvalidDiscard { count, buffer_len }` | Internal: tried to discard more bytes than are buffered |
| `Serde(serde_json::Error)` | JSON serialisation or deserialisation failure |

---

## Streaming behaviour

TCP is a byte stream — a single `read()` call can return a partial frame, multiple frames, or anything in between. The protocol handles this in two layers:

1. **`try_decode_frame`** — stateless function. Given a slice, it returns `Ok(None)` if the slice is too short (incomplete header or payload), a decoded value plus consumed-byte count on success, or an error for malformed data.

2. **`FrameBuffer`** — stateful wrapper. It accumulates raw bytes across multiple reads and removes exactly the bytes belonging to a complete frame when one is decoded. Bytes from subsequent frames are retained for the next `try_decode` call.

```
read #1: [0, 0, 0, 10, '{']          → try_decode → None  (5 of 14 bytes)
read #2: ['"', 'H', 'e', 'l', 'l']  → try_decode → None  (10 of 14 bytes)
read #3: ['o', '"', '}', 0, 0, 0, 2] → try_decode → Some(Hello { .. })
                                         buffer now holds [0, 0, 0, 2]
```
