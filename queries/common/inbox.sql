--! try_insert_inbox
-- Idempotent: returns true if inserted, false if already existed (dedup).
INSERT INTO common.inbox (event_id, event_type, source)
VALUES (:event_id, :event_type, :source)
ON CONFLICT (event_id) DO NOTHING;

--! check_processed
SELECT 1 FROM common.inbox WHERE event_id = :event_id;
