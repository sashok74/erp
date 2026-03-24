--! insert_domain_history
INSERT INTO common.domain_history
    (tenant_id, entity_type, entity_id, event_type,
     old_state, new_state,
     correlation_id, causation_id, user_id, created_at)
VALUES
    (:tenant_id, :entity_type, :entity_id, :event_type,
     :old_state, :new_state,
     :correlation_id, :causation_id, :user_id, :created_at)
RETURNING id;
