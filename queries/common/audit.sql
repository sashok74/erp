--! insert_audit_log
INSERT INTO common.audit_log
    (tenant_id, user_id, command_name, result,
     correlation_id, causation_id, created_at)
VALUES
    (:tenant_id, :user_id, :command_name, :result,
     :correlation_id, :causation_id, :created_at)
RETURNING id;
