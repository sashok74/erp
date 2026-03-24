-- V007: Inbox для idempotent event processing (deduplication).
-- При получении события: INSERT ON CONFLICT DO NOTHING.
-- Если уже обработано — skip.
CREATE TABLE IF NOT EXISTS common.inbox (
    event_id        UUID PRIMARY KEY,
    event_type      TEXT NOT NULL,
    source          TEXT NOT NULL,
    processed_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
