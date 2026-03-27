--! try_insert_inbox
-- Idempotent: returns 1 row affected if inserted, 0 if already existed (dedup).
INSERT INTO common.inbox (event_id, event_type, source, handler_name)
VALUES (:event_id, :event_type, :source, :handler_name)
ON CONFLICT (event_id, handler_name) DO NOTHING;

--! check_processed
SELECT 1 FROM common.inbox
WHERE event_id = :event_id AND handler_name = :handler_name;
