# hoy-net

Hoy chat app networking logic.

## Lib modules

- `client_id`: tiny strongly-typed client identifier
- `command`: commands sent from nonnection tasks into the central server state loop
- `connection`: per-connection read/write task plumbing
- `error`: networking errors
- `server.rs`: TCP listener, accept loop, central state loop and high-level orchestration

## Server design

- only one task owns chat state
- connection tasks never mutate global state directly
- connection tasks send commands into the server loop
- server loop sends packets back to clients through per-client `mpsc::Sender<ServerPacket>`

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
