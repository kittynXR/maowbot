-- File: migrations/20250202000000_make_chat_messages_partitioned_and_add_archive.sql

-- 1) Partitioned chat_messages:
CREATE TABLE IF NOT EXISTS chat_messages (
    message_id   TEXT NOT NULL,
    platform     TEXT NOT NULL,
    channel      TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    message_text TEXT NOT NULL,
    timestamp    BIGINT NOT NULL,
    metadata     TEXT,
    CONSTRAINT fk_userid FOREIGN KEY (user_id) REFERENCES users(user_id)
)
PARTITION BY RANGE (timestamp);

-- Make (timestamp, message_id) the primary key:
ALTER TABLE ONLY chat_messages
    ADD CONSTRAINT chat_messages_pk
    PRIMARY KEY (timestamp, message_id);

CREATE INDEX IF NOT EXISTS idx_chat_messages_platform_channel_timestamp
    ON chat_messages (platform, channel, timestamp);

-- 2) A default partition for all timestamps not handled elsewhere:
CREATE TABLE IF NOT EXISTS chat_messages_default
    PARTITION OF chat_messages
    DEFAULT;

-- 3) This separate table is used by your archiving logic:
CREATE TABLE IF NOT EXISTS chat_messages_archive (
    message_id   TEXT PRIMARY KEY,
    platform     TEXT NOT NULL,
    channel      TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    message_text TEXT NOT NULL,
    timestamp    BIGINT NOT NULL,
    metadata     TEXT
);
