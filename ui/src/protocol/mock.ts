import type {
  NavigationDescriptor,
  ScreenDescriptor,
  DataPayload,
  BehaviorState,
} from "./types";

// ─── Navigation (OutlookBar) ───────────────────────────────────

export const mockNavigation: NavigationDescriptor = {
  groups: [
    {
      id: "catalog",
      label: "Каталог",
      icon: "package",
      items: [
        { id: "product_list", label: "Продукция", screen_id: "catalog.product_list" },
      ],
    },
    {
      id: "warehouse",
      label: "Склад",
      icon: "warehouse",
      items: [
        { id: "receipt_list", label: "Приходные ордера", screen_id: "warehouse.receipt_list" },
        { id: "balance", label: "Остатки", screen_id: "warehouse.balance" },
      ],
    },
    {
      id: "admin",
      label: "Администрирование",
      icon: "settings",
      items: [
        { id: "tenant_list", label: "Тенанты", screen_id: "admin.tenant_list" },
        { id: "user_list", label: "Пользователи", screen_id: "admin.user_list" },
        { id: "role_list", label: "Роли и права", screen_id: "admin.role_list" },
      ],
    },
  ],
};

// ─── Screens ───────────────────────────────────────────────────

export const mockScreens: Record<string, ScreenDescriptor> = {
  "catalog.product_list": {
    id: "catalog.product_list",
    title: "Продукция",
    kind: "list",
    sections: [
      {
        type: "collection",
        def: {
          id: "products",
          detail_screen: "catalog.product_card",
          columns: [
            { id: "sku", label: "Артикул", field_type: "text", width: 150, pinned: "left" },
            { id: "name", label: "Наименование", field_type: "text", width: 300 },
            { id: "category", label: "Категория", field_type: "text", width: 150 },
            { id: "unit", label: "Ед. изм.", field_type: "text", width: 80 },
          ],
        },
      },
    ],
    actions: [
      { id: "create", label: "Создать", command: "catalog.create_product", icon: "plus", hotkey: "Ctrl+N", position: "toolbar", group: "main" },
      { id: "refresh", label: "Обновить", command: "", icon: "refresh-cw", position: "toolbar", group: "main" },
    ],
  },

  "warehouse.receipt_list": {
    id: "warehouse.receipt_list",
    title: "Приходные ордера",
    kind: "list",
    sections: [
      {
        type: "collection",
        def: {
          id: "receipts",
          detail_screen: "warehouse.receipt_card",
          columns: [
            { id: "doc_number", label: "Номер", field_type: "text", width: 150, pinned: "left" },
            { id: "date", label: "Дата", field_type: "date", width: 120 },
            { id: "storage", label: "Склад", field_type: "text", width: 200 },
            { id: "status", label: "Статус", field_type: "enum", width: 120 },
            { id: "total", label: "Сумма", field_type: "number", width: 120 },
          ],
        },
      },
    ],
    actions: [
      { id: "create", label: "Создать", command: "warehouse.create_receipt", icon: "plus", hotkey: "Ctrl+N", position: "toolbar", group: "main" },
      { id: "refresh", label: "Обновить", command: "", icon: "refresh-cw", position: "toolbar", group: "main" },
    ],
  },

  "warehouse.receipt_card": {
    id: "warehouse.receipt_card",
    title: "Приходный ордер",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "header",
          columns: 3,
          fields: [
            { id: "doc_number", label: "Номер", field_type: "text" },
            { id: "date", label: "Дата", field_type: "date" },
            { id: "storage", label: "Склад", field_type: "lookup", lookup: { entity: "storage", display_field: "name" } },
            { id: "contragent", label: "Поставщик", field_type: "lookup", lookup: { entity: "contragent", display_field: "name" } },
            { id: "note", label: "Примечание", field_type: "textarea" },
          ],
        },
      },
      {
        type: "collection",
        def: {
          id: "lines",
          title: "Строки документа",
          columns: [
            { id: "nomenclatura", label: "Номенклатура", field_type: "lookup", width: 250, lookup: { entity: "nomenclatura", display_field: "name" } },
            { id: "unit", label: "Ед.", field_type: "text", width: 60 },
            { id: "quantity", label: "Кол-во", field_type: "number", width: 100 },
            { id: "price", label: "Цена", field_type: "number", width: 100 },
            { id: "total", label: "Итого", field_type: "number", width: 120, computed: "quantity * price" },
          ],
        },
      },
    ],
    actions: [
      { id: "save", label: "Сохранить", command: "warehouse.save_receipt", icon: "save", hotkey: "Ctrl+S", position: "toolbar", group: "main" },
      { id: "execute", label: "Провести", command: "warehouse.execute_receipt", icon: "check", hotkey: "Ctrl+Enter", position: "toolbar", group: "main" },
      { id: "unexecute", label: "Отменить проведение", command: "warehouse.unexecute_receipt", icon: "undo", position: "toolbar", group: "main" },
      { id: "delete", label: "Удалить", command: "warehouse.delete_receipt", icon: "trash-2", confirm: "Удалить документ?", position: "toolbar", group: "danger" },
    ],
  },

  "warehouse.balance": {
    id: "warehouse.balance",
    title: "Остатки по складу",
    kind: "list",
    sections: [
      {
        type: "collection",
        def: {
          id: "balances",
          columns: [
            { id: "sku", label: "Артикул", field_type: "text", width: 150 },
            { id: "name", label: "Наименование", field_type: "text", width: 300 },
            { id: "storage", label: "Склад", field_type: "text", width: 200 },
            { id: "balance", label: "Остаток", field_type: "number", width: 120 },
            { id: "unit", label: "Ед. изм.", field_type: "text", width: 80 },
          ],
        },
      },
    ],
    actions: [
      { id: "refresh", label: "Обновить", command: "", icon: "refresh-cw", position: "toolbar", group: "main" },
    ],
  },

  // ─── Admin: Tenants ────────────────────────────────────────────

  "admin.tenant_list": {
    id: "admin.tenant_list",
    title: "Тенанты",
    kind: "list",
    sections: [
      {
        type: "collection",
        def: {
          id: "tenants",
          detail_screen: "admin.tenant_card",
          columns: [
            { id: "name", label: "Название", field_type: "text", width: 250 },
            { id: "slug", label: "Slug", field_type: "text", width: 150 },
            { id: "is_active", label: "Статус", field_type: "enum", width: 120 },
            { id: "created_at", label: "Создан", field_type: "date", width: 150 },
          ],
        },
      },
    ],
    actions: [
      { id: "create", label: "Создать", command: "admin.create_tenant", icon: "plus", hotkey: "Ctrl+N", position: "toolbar", group: "main" },
      { id: "refresh", label: "Обновить", command: "", icon: "refresh-cw", position: "toolbar", group: "main" },
    ],
  },

  "admin.tenant_card": {
    id: "admin.tenant_card",
    title: "Тенант",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "tenant",
          columns: 2,
          fields: [
            { id: "name", label: "Название", field_type: "text" },
            { id: "slug", label: "Slug", field_type: "text" },
            { id: "is_active", label: "Активен", field_type: "bool" },
            { id: "created_at", label: "Создан", field_type: "date" },
            { id: "updated_at", label: "Обновлён", field_type: "date" },
          ],
        },
      },
    ],
    actions: [
      { id: "save", label: "Сохранить", command: "admin.save_tenant", icon: "save", hotkey: "Ctrl+S", position: "toolbar", group: "main" },
      { id: "deactivate", label: "Деактивировать", command: "admin.deactivate_tenant", icon: "power-off", confirm: "Деактивировать тенант?", position: "toolbar", group: "danger" },
      { id: "activate", label: "Активировать", command: "admin.activate_tenant", icon: "power", position: "toolbar", group: "main" },
    ],
  },

  // ─── Admin: Roles ──────────────────────────────────────────────

  "admin.role_list": {
    id: "admin.role_list",
    title: "Роли и права",
    kind: "list",
    sections: [
      {
        type: "collection",
        def: {
          id: "roles",
          columns: [
            { id: "code", label: "Код", field_type: "text", width: 180 },
            { id: "display_name_ru", label: "Название", field_type: "text", width: 200 },
            { id: "bc", label: "Модуль", field_type: "text", width: 120 },
            { id: "security_level", label: "Уровень", field_type: "number", width: 80 },
            { id: "is_superadmin", label: "Суперадмин", field_type: "enum", width: 120 },
          ],
        },
      },
    ],
    actions: [
      { id: "refresh", label: "Обновить", command: "", icon: "refresh-cw", position: "toolbar", group: "main" },
    ],
  },

  // ─── Admin: Users ──────────────────────────────────────────────

  "admin.user_list": {
    id: "admin.user_list",
    title: "Пользователи",
    kind: "list",
    sections: [
      {
        type: "collection",
        def: {
          id: "users",
          detail_screen: "admin.user_card",
          columns: [
            { id: "username", label: "Логин", field_type: "text", width: 140, pinned: "left" },
            { id: "display_name", label: "Имя", field_type: "text", width: 200 },
            { id: "email", label: "Email", field_type: "text", width: 200 },
            { id: "is_active", label: "Статус", field_type: "enum", width: 100 },
            { id: "tenant_count", label: "Тенантов", field_type: "number", width: 90 },
            { id: "created_at", label: "Создан", field_type: "date", width: 120 },
          ],
        },
      },
    ],
    actions: [
      { id: "create", label: "Создать", command: "admin.create_user", icon: "plus", hotkey: "Ctrl+N", position: "toolbar", group: "main" },
      { id: "refresh", label: "Обновить", command: "", icon: "refresh-cw", position: "toolbar", group: "main" },
    ],
  },

  "admin.user_card": {
    id: "admin.user_card",
    title: "Пользователь",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "user_info",
          columns: 2,
          fields: [
            { id: "username", label: "Логин", field_type: "text" },
            { id: "display_name", label: "Отображаемое имя", field_type: "text" },
            { id: "email", label: "Email", field_type: "text" },
            { id: "is_active", label: "Активен", field_type: "bool" },
            { id: "created_at", label: "Создан", field_type: "date" },
            { id: "updated_at", label: "Обновлён", field_type: "date" },
          ],
        },
      },
      {
        type: "tabs",
        def: {
          id: "user_tabs",
          tabs: [
            {
              id: "tenant_roles",
              label: "Тенанты и роли",
              sections: [
                {
                  type: "collection",
                  def: {
                    id: "tenant_roles",
                    columns: [
                      { id: "tenant_name", label: "Организация", field_type: "text", width: 200 },
                      { id: "roles", label: "Роли", field_type: "text", width: 300 },
                      { id: "is_primary", label: "Основной", field_type: "bool", width: 80 },
                    ],
                  },
                },
              ],
            },
            {
              id: "audit",
              label: "Аудит",
              sections: [
                {
                  type: "collection",
                  def: {
                    id: "audit_log",
                    columns: [
                      { id: "timestamp", label: "Дата", field_type: "date", width: 160 },
                      { id: "action", label: "Действие", field_type: "text", width: 200 },
                      { id: "details", label: "Детали", field_type: "text", width: 300 },
                    ],
                  },
                },
              ],
            },
          ],
        },
      },
    ],
    actions: [
      { id: "save", label: "Сохранить", command: "admin.save_user", icon: "save", hotkey: "Ctrl+S", position: "toolbar", group: "main" },
      { id: "reset_password", label: "Сбросить пароль", command: "admin.reset_password", icon: "key", confirm: "Сбросить пароль пользователя?", position: "toolbar", group: "main" },
      { id: "deactivate", label: "Деактивировать", command: "admin.deactivate_user", icon: "power-off", confirm: "Деактивировать пользователя?", position: "toolbar", group: "danger" },
      { id: "activate", label: "Активировать", command: "admin.activate_user", icon: "power", position: "toolbar", group: "main" },
    ],
  },
};

// ─── Mock Data ─────────────────────────────────────────────────

export const mockData: Record<string, DataPayload> = {
  "catalog.product_list": {
    collections: {
      products: {
        total_count: 5,
        rows: [
          { sku: "BOLT-M8", name: "Болт М8×40", category: "Метизы", unit: "шт" },
          { sku: "NUT-M8", name: "Гайка М8", category: "Метизы", unit: "шт" },
          { sku: "PIPE-50", name: "Труба 50мм", category: "Трубы", unit: "м" },
          { sku: "CABLE-4", name: "Кабель ВВГ 4×2.5", category: "Электрика", unit: "м" },
          { sku: "PAINT-W", name: "Краска белая", category: "ЛКМ", unit: "кг" },
        ],
      },
    },
  },

  "warehouse.receipt_list": {
    collections: {
      receipts: {
        total_count: 4,
        rows: [
          { doc_number: "ПРХ-000001", date: "2026-04-01", storage: "Склад №1", status: "Проведён", total: 45000.0 },
          { doc_number: "ПРХ-000002", date: "2026-04-02", storage: "Склад №1", status: "Черновик", total: 12800.0 },
          { doc_number: "ПРХ-000003", date: "2026-04-03", storage: "Склад №2", status: "Черновик", total: 7300.0 },
          { doc_number: "ПРХ-000004", date: "2026-04-03", storage: "Склад №1", status: "Проведён", total: 91200.0 },
        ],
      },
    },
  },

  "warehouse.receipt_card": {
    record: {
      id: "rec-002",
      doc_number: "ПРХ-000002",
      date: "2026-04-02",
      storage: "Склад №1",
      contragent: "ООО СтройМат",
      note: "",
      status: "draft",
    },
    collections: {
      lines: {
        total_count: 3,
        rows: [
          { id: "1", nomenclatura: "Болт М8×40", unit: "шт", quantity: 500, price: 12.0, total: 6000.0 },
          { id: "2", nomenclatura: "Гайка М8", unit: "шт", quantity: 500, price: 8.0, total: 4000.0 },
          { id: "3", nomenclatura: "Труба 50мм", unit: "м", quantity: 20, price: 140.0, total: 2800.0 },
        ],
      },
    },
  },

  "warehouse.balance": {
    collections: {
      balances: {
        total_count: 4,
        rows: [
          { sku: "BOLT-M8", name: "Болт М8×40", storage: "Склад №1", balance: 1200, unit: "шт" },
          { sku: "NUT-M8", name: "Гайка М8", storage: "Склад №1", balance: 950, unit: "шт" },
          { sku: "PIPE-50", name: "Труба 50мм", storage: "Склад №2", balance: 85, unit: "м" },
          { sku: "CABLE-4", name: "Кабель ВВГ 4×2.5", storage: "Склад №1", balance: 320, unit: "м" },
        ],
      },
    },
  },

  // ─── Admin: Tenants ──────────────────────────────────────────

  "admin.tenant_list": {
    collections: {
      tenants: {
        total_count: 3,
        rows: [
          { id: "019d0001-0000-7000-8000-000000000001", name: "Демо Компания", slug: "demo", is_active: "Активен", created_at: "2026-01-15" },
          { id: "019d0001-0000-7000-8000-000000000002", name: "СтройМонтаж", slug: "stroymontazh", is_active: "Активен", created_at: "2026-02-20" },
          { id: "019d0001-0000-7000-8000-000000000003", name: "Тест (архив)", slug: "test-archive", is_active: "Неактивен", created_at: "2025-11-01" },
        ],
      },
    },
  },

  "admin.tenant_card": {
    record: {
      id: "019d0001-0000-7000-8000-000000000001",
      name: "Демо Компания",
      slug: "demo",
      is_active: true,
      status: "active",
      created_at: "2026-01-15",
      updated_at: "2026-04-01",
    },
  },

  // ─── Admin: Roles ────────────────────────────────────────────

  "admin.role_list": {
    collections: {
      roles: {
        total_count: 5,
        rows: [
          { code: "admin", display_name_ru: "Администратор", bc: "platform", security_level: 255, is_superadmin: "Да" },
          { code: "viewer", display_name_ru: "Наблюдатель", bc: "platform", security_level: 0, is_superadmin: "Нет" },
          { code: "catalog_manager", display_name_ru: "Менеджер каталога", bc: "catalog", security_level: 1, is_superadmin: "Нет" },
          { code: "warehouse_manager", display_name_ru: "Менеджер склада", bc: "warehouse", security_level: 2, is_superadmin: "Нет" },
          { code: "warehouse_operator", display_name_ru: "Кладовщик", bc: "warehouse", security_level: 1, is_superadmin: "Нет" },
        ],
      },
    },
  },

  // ─── Admin: Users ────────────────────────────────────────────

  "admin.user_list": {
    collections: {
      users: {
        total_count: 6,
        rows: [
          { username: "admin", display_name: "Администратор А.А.", email: "admin@erp.local", is_active: "Активен", tenant_count: 3, created_at: "2026-01-10" },
          { username: "ivanov", display_name: "Иванов И.И.", email: "ivanov@erp.local", is_active: "Активен", tenant_count: 1, created_at: "2026-02-15" },
          { username: "petrova", display_name: "Петрова М.С.", email: "petrova@erp.local", is_active: "Активен", tenant_count: 2, created_at: "2026-02-20" },
          { username: "sidorov", display_name: "Сидоров К.В.", email: "sidorov@erp.local", is_active: "Активен", tenant_count: 1, created_at: "2026-03-01" },
          { username: "smirnova", display_name: "Смирнова Е.Л.", email: "smirnova@erp.local", is_active: "Активен", tenant_count: 1, created_at: "2026-03-10" },
          { username: "inactive", display_name: "Неактивный П.П.", email: "inactive@erp.local", is_active: "Неактивен", tenant_count: 0, created_at: "2025-11-01" },
        ],
      },
    },
  },

  "admin.user_card": {
    record: {
      id: "u-002",
      username: "ivanov",
      display_name: "Иванов И.И.",
      email: "ivanov@erp.local",
      is_active: true,
      status: "active",
      created_at: "2026-02-15",
      updated_at: "2026-04-01",
    },
    collections: {
      tenant_roles: {
        total_count: 1,
        rows: [
          { tenant_name: "Демо Компания", roles: "Менеджер склада", is_primary: true },
        ],
      },
      audit_log: {
        total_count: 3,
        rows: [
          { timestamp: "2026-04-01 09:15", action: "Вход в систему", details: "Тенант: Демо Компания" },
          { timestamp: "2026-03-28 14:30", action: "Изменение профиля", details: "Email обновлён" },
          { timestamp: "2026-02-15 10:00", action: "Создание пользователя", details: "Создан администратором" },
        ],
      },
    },
  },
};

// ─── Mock Behavior ─────────────────────────────────────────────

export const mockBehavior: Record<string, BehaviorState> = {
  "catalog.product_list": {
    fields: {},
    actions: {
      create: { visible: true, enabled: true },
      refresh: { visible: true, enabled: true },
    },
  },

  "warehouse.receipt_list": {
    fields: {},
    actions: {
      create: { visible: true, enabled: true },
      refresh: { visible: true, enabled: true },
    },
  },

  "warehouse.receipt_card": {
    fields: {
      doc_number: { visible: true, editable: false, required: false },
      date: { visible: true, editable: true, required: true },
      storage: { visible: true, editable: true, required: true },
      contragent: { visible: true, editable: true, required: false },
      note: { visible: true, editable: true, required: false },
    },
    actions: {
      save: { visible: true, enabled: true },
      execute: { visible: true, enabled: true },
      unexecute: { visible: false, enabled: false },
      delete: { visible: true, enabled: true },
    },
    collections: {
      lines: { editable: true, can_add: true, can_delete: true },
    },
  },

  "warehouse.balance": {
    fields: {},
    actions: {
      refresh: { visible: true, enabled: true },
    },
  },

  // ─── Admin ───────────────────────────────────────────────────

  "admin.tenant_list": {
    fields: {},
    actions: {
      create: { visible: true, enabled: true },
      refresh: { visible: true, enabled: true },
    },
  },

  "admin.tenant_card": {
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
  },

  "admin.role_list": {
    fields: {},
    actions: {
      refresh: { visible: true, enabled: true },
    },
  },

  "admin.user_list": {
    fields: {},
    actions: {
      create: { visible: true, enabled: true },
      refresh: { visible: true, enabled: true },
    },
  },

  "admin.user_card": {
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
  },
};
