import type {
  NavigationDescriptor,
  ScreenDescriptor,
  BehaviorState,
} from "./types";

// ─── Navigation (only what backend supports) ──────────────────

export const mockNavigation: NavigationDescriptor = {
  groups: [
    {
      id: "catalog",
      label: "Каталог",
      icon: "package",
      items: [
        { id: "create_product", label: "Создать продукт", screen_id: "catalog.create_product" },
        { id: "find_product", label: "Найти продукт", screen_id: "catalog.find_product" },
      ],
    },
    {
      id: "warehouse",
      label: "Склад",
      icon: "warehouse",
      items: [
        { id: "receive", label: "Приёмка товара", screen_id: "warehouse.receive" },
        { id: "balance", label: "Проверить остаток", screen_id: "warehouse.balance" },
      ],
    },
  ],
};

// ─── Screen descriptors ────────────────────────────────────────

export const mockScreens: Record<string, ScreenDescriptor> = {
  "catalog.create_product": {
    id: "catalog.create_product",
    title: "Создать продукт",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "product",
          columns: 2,
          fields: [
            { id: "sku", label: "Артикул (SKU)", field_type: "text" },
            { id: "name", label: "Наименование", field_type: "text" },
            { id: "category", label: "Категория", field_type: "text" },
            { id: "unit", label: "Единица измерения", field_type: "text" },
          ],
        },
      },
    ],
    actions: [
      { id: "create", label: "Создать", command: "catalog.create_product", icon: "plus", hotkey: "Ctrl+Enter", position: "toolbar", group: "main" },
    ],
  },

  "catalog.find_product": {
    id: "catalog.find_product",
    title: "Найти продукт",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "search",
          columns: 1,
          fields: [
            { id: "sku", label: "Артикул (SKU)", field_type: "text" },
          ],
        },
      },
      {
        type: "record",
        def: {
          id: "result",
          title: "Результат",
          columns: 2,
          fields: [
            { id: "product_id", label: "ID", field_type: "text" },
            { id: "sku_result", label: "Артикул", field_type: "text" },
            { id: "name", label: "Наименование", field_type: "text" },
            { id: "category", label: "Категория", field_type: "text" },
            { id: "unit", label: "Единица", field_type: "text" },
          ],
        },
      },
    ],
    actions: [
      { id: "search", label: "Найти", command: "catalog.get_product", icon: "refresh-cw", hotkey: "Enter", position: "toolbar", group: "main" },
    ],
  },

  "warehouse.receive": {
    id: "warehouse.receive",
    title: "Приёмка товара",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "receive",
          columns: 2,
          fields: [
            { id: "sku", label: "Артикул (SKU)", field_type: "text" },
            { id: "quantity", label: "Количество", field_type: "number" },
          ],
        },
      },
      {
        type: "record",
        def: {
          id: "result",
          title: "Результат приёмки",
          columns: 2,
          fields: [
            { id: "doc_number", label: "Номер документа", field_type: "text" },
            { id: "new_balance", label: "Новый остаток", field_type: "text" },
            { id: "item_id", label: "ID позиции", field_type: "text" },
            { id: "movement_id", label: "ID движения", field_type: "text" },
          ],
        },
      },
    ],
    actions: [
      { id: "receive", label: "Принять", command: "warehouse.receive_goods", icon: "check", hotkey: "Ctrl+Enter", position: "toolbar", group: "main" },
    ],
  },

  "warehouse.balance": {
    id: "warehouse.balance",
    title: "Проверить остаток",
    kind: "card",
    sections: [
      {
        type: "record",
        def: {
          id: "search",
          columns: 1,
          fields: [
            { id: "sku", label: "Артикул (SKU)", field_type: "text" },
          ],
        },
      },
      {
        type: "record",
        def: {
          id: "result",
          title: "Остаток",
          columns: 2,
          fields: [
            { id: "sku_result", label: "Артикул", field_type: "text" },
            { id: "balance", label: "Остаток", field_type: "number" },
            { id: "product_name", label: "Наименование", field_type: "text" },
            { id: "item_id", label: "ID позиции", field_type: "text" },
          ],
        },
      },
    ],
    actions: [
      { id: "search", label: "Проверить", command: "warehouse.get_balance", icon: "refresh-cw", hotkey: "Enter", position: "toolbar", group: "main" },
    ],
  },
};

// ─── Behavior ──────────────────────────────────────────────────

export const mockBehavior: Record<string, BehaviorState> = {
  "catalog.create_product": {
    fields: {
      sku:  { visible: true, editable: true, required: true },
      name: { visible: true, editable: true, required: true },
      category: { visible: true, editable: true, required: true },
      unit: { visible: true, editable: true, required: true },
    },
    actions: {
      create: { visible: true, enabled: true },
    },
  },

  "catalog.find_product": {
    fields: {
      sku: { visible: true, editable: true, required: true },
      product_id: { visible: true, editable: false, required: false },
      sku_result: { visible: true, editable: false, required: false },
      name: { visible: true, editable: false, required: false },
      category: { visible: true, editable: false, required: false },
      unit: { visible: true, editable: false, required: false },
    },
    actions: {
      search: { visible: true, enabled: true },
    },
  },

  "warehouse.receive": {
    fields: {
      sku:      { visible: true, editable: true, required: true },
      quantity: { visible: true, editable: true, required: true },
      doc_number:  { visible: true, editable: false, required: false },
      new_balance: { visible: true, editable: false, required: false },
      item_id:     { visible: true, editable: false, required: false },
      movement_id: { visible: true, editable: false, required: false },
    },
    actions: {
      receive: { visible: true, enabled: true },
    },
  },

  "warehouse.balance": {
    fields: {
      sku: { visible: true, editable: true, required: true },
      sku_result:    { visible: true, editable: false, required: false },
      balance:       { visible: true, editable: false, required: false },
      product_name:  { visible: true, editable: false, required: false },
      item_id:       { visible: true, editable: false, required: false },
    },
    actions: {
      search: { visible: true, enabled: true },
    },
  },
};
