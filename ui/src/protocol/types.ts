// ─── Core IDs ──────────────────────────────────────────────────

export type ScreenId = string; // "warehouse.receipt_list"
export type FieldId = string;
export type ActionId = string;

// ─── Contract #1: ScreenDescriptor ─────────────────────────────
// What to render. Loaded once per screen open.

export interface ScreenDescriptor {
  id: ScreenId;
  title: string;
  kind: ScreenKind;
  sections: SectionDef[];
  actions: ActionDef[];
}

export type ScreenKind = "list" | "card" | "tree";

export type SectionDef =
  | { type: "record"; def: RecordDef }
  | { type: "collection"; def: CollectionDef }
  | { type: "tabs"; def: TabsDef };

export interface RecordDef {
  id: string;
  title?: string;
  columns: 1 | 2 | 3;
  fields: FieldDef[];
}

export interface CollectionDef {
  id: string;
  title?: string;
  columns: ColumnDef[];
  row_actions?: ActionDef[];
  detail_screen?: ScreenId;
}

export interface TabsDef {
  id: string;
  tabs: Array<{
    id: string;
    label: string;
    sections: SectionDef[];
  }>;
}

export interface FieldDef {
  id: FieldId;
  label: string;
  field_type: FieldType;
  lookup?: LookupDef;
  computed?: string;
}

export type FieldType =
  | "text"
  | "number"
  | "date"
  | "bool"
  | "enum"
  | "lookup"
  | "textarea";

export interface LookupDef {
  entity: string;
  display_field: string;
  search_endpoint?: string;
}

export interface ColumnDef {
  id: string;
  label: string;
  field_type: FieldType;
  width?: number;
  pinned?: "left" | "right";
  lookup?: LookupDef;
  computed?: string;
}

export interface ActionDef {
  id: ActionId;
  label: string;
  command: string;
  icon?: string;
  hotkey?: string;
  confirm?: string;
  position: ActionPosition;
  group?: string;
}

export type ActionPosition = "toolbar" | "row" | "context_menu";

// ─── Contract #2: DataPayload ──────────────────────────────────
// The data to display. Loaded separately, refreshable.

export interface DataPayload {
  record?: Record<string, unknown>;
  collections?: Record<
    string,
    {
      rows: Record<string, unknown>[];
      total_count: number;
    }
  >;
}

// ─── Contract #3: BehaviorState ────────────────────────────────
// What is allowed. Changes when data/status/role changes.

export interface BehaviorState {
  fields: Record<FieldId, FieldState>;
  actions: Record<ActionId, ActionState>;
  collections?: Record<string, CollectionState>;
  validations?: ValidationMessage[];
}

export interface FieldState {
  visible: boolean;
  editable: boolean;
  required: boolean;
}

export interface ActionState {
  visible: boolean;
  enabled: boolean;
  reason?: string;
}

export interface CollectionState {
  editable: boolean;
  can_add: boolean;
  can_delete: boolean;
  editable_columns?: string[];
}

export interface ValidationMessage {
  field?: string;
  level: "error" | "warning" | "info";
  message: string;
}

// ─── UiEvent (client → server) ─────────────────────────────────

export type UiEvent =
  | { type: "action_invoked"; action: ActionId; payload?: unknown }
  | { type: "field_changed"; field: FieldId; value: unknown }
  | { type: "row_added"; collection: string }
  | { type: "row_deleted"; collection: string; row_id: string }
  | { type: "row_changed"; collection: string; row_id: string; field: string; value: unknown };

// ─── UiEffect (server → client) ────────────────────────────────

export type UiEffect =
  | { type: "refresh" }
  | { type: "refresh_section"; section: string }
  | { type: "toast"; level: "success" | "error" | "warning" | "info"; message: string }
  | { type: "navigate"; screen_id: ScreenId; params?: Record<string, string> }
  | { type: "close_form" }
  | { type: "open_dialog"; dialog: string }
  | { type: "update_behavior"; behavior: BehaviorState }
  | { type: "update_data"; data: Partial<DataPayload> };

// ─── Navigation ────────────────────────────────────────────────

export interface NavigationDescriptor {
  groups: NavGroup[];
}

export interface NavGroup {
  id: string;
  label: string;
  icon: string;
  items: NavItem[];
}

export interface NavItem {
  id: string;
  label: string;
  screen_id: ScreenId;
  icon?: string;
  badge?: number;
}
