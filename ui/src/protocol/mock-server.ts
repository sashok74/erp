import type { DataPayload, BehaviorState, UiEffect } from "./types";

export interface ActionResult {
  data?: DataPayload;
  behavior?: BehaviorState;
  effects: UiEffect[];
}

/**
 * Simulates server-side action handling.
 * In production this is POST /ui/screen/{id}/action → ui_runtime resolves new state.
 */
export function handleAction(
  screenId: string,
  actionId: string,
  currentData: DataPayload,
): ActionResult {
  const key = `${screenId}::${actionId}`;

  switch (key) {
    case "warehouse.receipt_card::execute":
      return {
        data: patchRecord(currentData, { status: "posted" }),
        behavior: receiptPostedBehavior(),
        effects: [
          { type: "toast", level: "success", message: "Документ проведён" },
        ],
      };

    case "warehouse.receipt_card::unexecute":
      return {
        data: patchRecord(currentData, { status: "draft" }),
        behavior: receiptDraftBehavior(),
        effects: [
          { type: "toast", level: "info", message: "Проведение отменено" },
        ],
      };

    case "warehouse.receipt_card::save":
      return {
        effects: [
          { type: "toast", level: "success", message: "Сохранено" },
        ],
      };

    case "warehouse.receipt_card::delete":
      return {
        effects: [
          { type: "toast", level: "warning", message: "Документ удалён" },
          { type: "close_form" },
        ],
      };

    // ─── Tenant ──────────────────────────────────────────────

    case "admin.tenant_card::save":
      return {
        effects: [
          { type: "toast", level: "success", message: "Тенант сохранён" },
        ],
      };

    case "admin.tenant_card::deactivate":
      return {
        data: patchRecord(currentData, { is_active: false, status: "inactive" }),
        behavior: tenantInactiveBehavior(),
        effects: [
          { type: "toast", level: "warning", message: "Тенант деактивирован" },
        ],
      };

    case "admin.tenant_card::activate":
      return {
        data: patchRecord(currentData, { is_active: true, status: "active" }),
        behavior: tenantActiveBehavior(),
        effects: [
          { type: "toast", level: "success", message: "Тенант активирован" },
        ],
      };

    // ─── User ────────────────────────────────────────────────

    case "admin.user_card::save":
      return {
        effects: [
          { type: "toast", level: "success", message: "Пользователь сохранён" },
        ],
      };

    case "admin.user_card::deactivate":
      return {
        data: patchRecord(currentData, { is_active: false, status: "inactive" }),
        behavior: userInactiveBehavior(),
        effects: [
          { type: "toast", level: "warning", message: "Пользователь деактивирован" },
        ],
      };

    case "admin.user_card::activate":
      return {
        data: patchRecord(currentData, { is_active: true, status: "active" }),
        behavior: userActiveBehavior(),
        effects: [
          { type: "toast", level: "success", message: "Пользователь активирован" },
        ],
      };

    case "admin.user_card::reset_password":
      return {
        effects: [
          { type: "toast", level: "success", message: "Пароль сброшен. Новый пароль отправлен на email." },
        ],
      };

    default:
      return {
        effects: [
          { type: "toast", level: "info", message: `Action: ${actionId}` },
        ],
      };
  }
}

// ─── Behavior presets ──────────────────────────────────────────

function receiptDraftBehavior(): BehaviorState {
  return {
    fields: {
      doc_number: { visible: true, editable: false, required: false },
      date:       { visible: true, editable: true,  required: true },
      storage:    { visible: true, editable: true,  required: true },
      contragent: { visible: true, editable: true,  required: false },
      note:       { visible: true, editable: true,  required: false },
    },
    actions: {
      save:      { visible: true,  enabled: true },
      execute:   { visible: true,  enabled: true },
      unexecute: { visible: false, enabled: false },
      delete:    { visible: true,  enabled: true },
    },
    collections: {
      lines: { editable: true, can_add: true, can_delete: true },
    },
  };
}

function receiptPostedBehavior(): BehaviorState {
  return {
    fields: {
      doc_number: { visible: true, editable: false, required: false },
      date:       { visible: true, editable: false, required: false },
      storage:    { visible: true, editable: false, required: false },
      contragent: { visible: true, editable: false, required: false },
      note:       { visible: true, editable: false, required: false },
    },
    actions: {
      save:      { visible: false, enabled: false },
      execute:   { visible: false, enabled: false },
      unexecute: { visible: true,  enabled: true },
      delete:    { visible: false, enabled: false },
    },
    collections: {
      lines: { editable: false, can_add: false, can_delete: false },
    },
  };
}

// ─── Tenant behavior presets ───────────────────────────────────

function tenantActiveBehavior(): BehaviorState {
  return {
    fields: {
      name:       { visible: true, editable: true,  required: true },
      slug:       { visible: true, editable: false, required: false },
      is_active:  { visible: true, editable: false, required: false },
      created_at: { visible: true, editable: false, required: false },
      updated_at: { visible: true, editable: false, required: false },
    },
    actions: {
      save:       { visible: true,  enabled: true },
      deactivate: { visible: true,  enabled: true },
      activate:   { visible: false, enabled: false },
    },
  };
}

function tenantInactiveBehavior(): BehaviorState {
  return {
    fields: {
      name:       { visible: true, editable: false, required: false },
      slug:       { visible: true, editable: false, required: false },
      is_active:  { visible: true, editable: false, required: false },
      created_at: { visible: true, editable: false, required: false },
      updated_at: { visible: true, editable: false, required: false },
    },
    actions: {
      save:       { visible: false, enabled: false },
      deactivate: { visible: false, enabled: false },
      activate:   { visible: true,  enabled: true },
    },
  };
}

// ─── User behavior presets ─────────────────────────────────────

function userActiveBehavior(): BehaviorState {
  return {
    fields: {
      username:     { visible: true, editable: false, required: false },
      display_name: { visible: true, editable: true,  required: true },
      email:        { visible: true, editable: true,  required: true },
      is_active:    { visible: true, editable: false, required: false },
      created_at:   { visible: true, editable: false, required: false },
      updated_at:   { visible: true, editable: false, required: false },
    },
    actions: {
      save:           { visible: true,  enabled: true },
      reset_password: { visible: true,  enabled: true },
      deactivate:     { visible: true,  enabled: true },
      activate:       { visible: false, enabled: false },
    },
  };
}

function userInactiveBehavior(): BehaviorState {
  return {
    fields: {
      username:     { visible: true, editable: false, required: false },
      display_name: { visible: true, editable: false, required: false },
      email:        { visible: true, editable: false, required: false },
      is_active:    { visible: true, editable: false, required: false },
      created_at:   { visible: true, editable: false, required: false },
      updated_at:   { visible: true, editable: false, required: false },
    },
    actions: {
      save:           { visible: false, enabled: false },
      reset_password: { visible: false, enabled: false },
      deactivate:     { visible: false, enabled: false },
      activate:       { visible: true,  enabled: true },
    },
  };
}

// ─── Helpers ───────────────────────────────────────────────────

function patchRecord(
  current: DataPayload,
  patch: Record<string, unknown>,
): DataPayload {
  return {
    ...current,
    record: { ...current.record, ...patch },
  };
}
