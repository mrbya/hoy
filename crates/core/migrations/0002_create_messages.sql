CREATE TABLE IF NOT EXISTS messages (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id   INTEGER NOT NULL REFERENCES rooms(id),
    from_user TEXT    NOT NULL,
    text      TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_room_id ON messages(room_id);
