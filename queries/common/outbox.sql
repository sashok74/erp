--! insert_outbox_entry
INSERT INTO common.outbox
    (tenant_id, event_id, event_type, source, payload,
     correlation_id, causation_id, user_id, created_at)
VALUES
    (:tenant_id, :event_id, :event_type, :source, :payload::jsonb,
     :correlation_id, :causation_id, :user_id, :created_at)
RETURNING id;

--! get_unpublished_events
SELECT id, tenant_id, event_id, event_type, source, payload,
       correlation_id, causation_id, user_id, created_at, retry_count
FROM common.outbox
WHERE published = false
ORDER BY id
LIMIT :batch_size
FOR UPDATE SKIP LOCKED;

--! mark_published
UPDATE common.outbox
SET published = true, published_at = NOW()
WHERE id = :id;

--! increment_retry
UPDATE common.outbox
SET retry_count = retry_count + 1
WHERE id = :id;
