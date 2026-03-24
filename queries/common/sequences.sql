--! next_value
SELECT prefix, next_value
FROM common.sequences
WHERE tenant_id = :tenant_id AND seq_name = :seq_name
FOR UPDATE;

--! increment_sequence
UPDATE common.sequences
SET next_value = next_value + 1
WHERE tenant_id = :tenant_id AND seq_name = :seq_name;

--! ensure_sequence
INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value)
VALUES (:tenant_id, :seq_name, :prefix, 1)
ON CONFLICT (tenant_id, seq_name) DO NOTHING;
