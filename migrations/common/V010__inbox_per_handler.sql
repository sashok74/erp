-- V010: Inbox dedup per (event_id, handler_name).
--
-- Allows multiple handlers to process same event independently.
-- Old PK was event_id only — broke with >1 handler per event_type.
--
-- Inbox is operational dedup table, not business data.
-- Historical rows are disposable — TRUNCATE is safe.
-- Worst case after TRUNCATE: one-time re-processing of events
-- that were already handled but relay retries (handlers use UPSERT).

TRUNCATE common.inbox;

DROP TABLE common.inbox;

CREATE TABLE common.inbox (
    event_id        UUID NOT NULL,
    handler_name    TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    source          TEXT NOT NULL,
    processed_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (event_id, handler_name)
);
