# ERP UI Engine — руководство разработчика

**Дата**: 2026-04-04
**Версия**: 0.1.0
**Расположение**: `erp/ui/`

---

## 1. Архитектура: три контракта

UI построен на **трёх отдельных контрактах**. Это главный архитектурный принцип — нарушение разделения между ними является ошибкой.

```
┌──────────────────┐  ┌──────────────┐  ┌─────────────────┐
│ ScreenDescriptor │  │  DataPayload │  │  BehaviorState  │
│   ЧТО рисовать   │  │    ДАННЫЕ    │  │  ЧТО МОЖНО      │
│                  │  │              │  │                 │
│ • секции         │  │ • record     │  │ • fields: vis/  │
│ • поля           │  │ • collections│  │   edit/required │
│ • колонки        │  │              │  │ • actions: vis/ │
│ • действия       │  │              │  │   enabled       │
│ • layout         │  │              │  │ • validations   │
└────────┬─────────┘  └──────┬───────┘  └────────┬────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
                             │
                    renderScreen(...)
                             │
                           DOM
```

### 1.1. ScreenDescriptor — структура экрана

Описывает **что рисовать**: какие секции, какие поля, какие колонки, какие действия. Не содержит данных. Не содержит состояния доступности.

Файл: `src/protocol/types.ts`

```typescript
interface ScreenDescriptor {
  id: ScreenId;           // "catalog.create_product"
  title: string;          // "Создать продукт"
  kind: ScreenKind;       // "list" | "card" | "tree"
  sections: SectionDef[]; // секции экрана
  actions: ActionDef[];   // действия (toolbar/row/context_menu)
}
```

Секции бывают трёх типов:
- `record` — форма с полями (RecordDef)
- `collection` — таблица с колонками (CollectionDef)
- `tabs` — вкладки, содержащие другие секции (TabsDef)

### 1.2. DataPayload — данные

Бизнес-данные для заполнения экрана. Приходят из API отдельно от структуры.

```typescript
interface DataPayload {
  record?: Record<string, unknown>;     // одна запись
  collections?: Record<string, {        // коллекции (гриды)
    rows: Record<string, unknown>[];
    total_count: number;
  }>;
}
```

### 1.3. BehaviorState — что разрешено

Управляет видимостью, доступностью, обязательностью. Меняется при изменении данных, статуса, роли.

```typescript
interface BehaviorState {
  fields: Record<FieldId, FieldState>;       // поля
  actions: Record<ActionId, ActionState>;     // действия
  collections?: Record<string, CollectionState>;
  validations?: ValidationMessage[];
}
```

---

## 2. Как добавить новый экран

### Шаг 1. Определить ScreenDescriptor

В файле `src/protocol/mock.ts` добавить определение экрана:

```typescript
"mybc.my_screen": {
  id: "mybc.my_screen",
  title: "Мой экран",
  kind: "card",
  sections: [
    {
      type: "record",
      def: {
        id: "main_form",
        columns: 2,            // 1, 2 или 3 колонки
        fields: [
          { id: "name", label: "Название", field_type: "text" },
          { id: "quantity", label: "Количество", field_type: "number" },
          { id: "date", label: "Дата", field_type: "date" },
          { id: "active", label: "Активен", field_type: "bool" },
        ],
      },
    },
  ],
  actions: [
    {
      id: "save",
      label: "Сохранить",
      command: "mybc.save",
      icon: "save",
      hotkey: "Ctrl+S",
      position: "toolbar",
      group: "main",
    },
  ],
},
```

### Шаг 2. Определить BehaviorState

В том же файле `src/protocol/mock.ts`:

```typescript
"mybc.my_screen": {
  fields: {
    name:     { visible: true, editable: true,  required: true },
    quantity: { visible: true, editable: true,  required: false },
    date:     { visible: true, editable: false, required: false },
    active:   { visible: true, editable: false, required: false },
  },
  actions: {
    save: { visible: true, enabled: true },
  },
},
```

### Шаг 3. Добавить в навигацию

В `mockNavigation` → нужная группа → `items`:

```typescript
{ id: "my_screen", label: "Мой экран", screen_id: "mybc.my_screen" },
```

### Шаг 4. Обработать действие

В `src/main.ts` → функция `executeAction` → добавить case:

```typescript
case "mybc.my_screen::save": {
  const result = await api.post("/api/mybc/save", {
    name: vals["name"],
    quantity: Number(vals["quantity"]),
  });
  if (result.ok) {
    toast("success", "Сохранено");
  } else {
    toast("error", result.error);
  }
  break;
}
```

### Шаг 5. (Опционально) Добавить типизированный API-вызов

В `src/protocol/api.ts`:

```typescript
export async function saveMyEntity(req: { name: string; quantity: number }) {
  return api.post("/api/mybc/save", req);
}
```

---

## 3. Типы полей

| `field_type` | HTML | Для чего |
|---|---|---|
| `text` | `<input type="text">` | Строки, SKU, имена |
| `number` | `<input type="number">` | Числа, количества |
| `date` | `<input type="date">` | Даты |
| `bool` | `<input type="checkbox">` | Флаги вкл/выкл |
| `enum` | Badge в коллекции | Статусы, категории |
| `lookup` | `<input>` с placeholder | Выбор связанной сущности (будущее) |
| `textarea` | `<textarea rows="2">` | Многострочный текст |

---

## 4. Типы секций

### Record — форма

```typescript
{ type: "record", def: { id: "form", columns: 2, fields: [...] } }
```

Поля рендерятся в CSS Grid (1/2/3 колонки). Каждое поле получает:
- label + обязательность (`*`) из BehaviorState
- input/textarea/checkbox в зависимости от `field_type`
- `disabled` если `editable: false`

### Collection — таблица

```typescript
{ type: "collection", def: { id: "items", columns: [...], detail_screen?: "..." } }
```

Рендерится как HTML `<table>` со sticky header. Если задан `detail_screen` — double-click по строке открывает карточку.

### Tabs — вкладки

```typescript
{ type: "tabs", def: { id: "tabs", tabs: [{ id: "t1", label: "Tab", sections: [...] }] } }
```

Вложенные секции внутри вкладок.

---

## 5. Действия (ActionDef)

```typescript
interface ActionDef {
  id: string;              // уникальный ID действия
  label: string;           // текст кнопки
  command: string;         // имя команды backend
  icon?: string;           // иконка (из icons.ts)
  hotkey?: string;         // клавиша ("Ctrl+S")
  confirm?: string;        // текст подтверждения (window.confirm)
  position: "toolbar" | "row" | "context_menu";
  group?: "main" | "danger"; // группа в ribbon
}
```

Действия с `position: "toolbar"` отображаются в Ribbon. Группа `"danger"` отделяется разделителем и окрашивается красным.

---

## 6. Обработка действий — цикл

```
Пользователь нажал кнопку в Ribbon
    ↓
executeAction(actionId) в main.ts
    ↓
Проверка confirm (если задан)
    ↓
Сбор значений полей из DOM (getFormValues)
    ↓
Вызов реального API (fetch → /api/...)
    ↓
ApiResult<T>
    ├── ok → toast("success", ...) + обновить данные (mdi.updateActiveTab)
    └── error → toast("error", errorMessage)
```

Ключевой паттерн: действие → API → обновление DataPayload → перерендер.

---

## 7. Shell: компоненты оболочки

```
┌──────────────────────────────────────────────────────┐
│  Ribbon — действия из активной вкладки               │
├──────────┬───────────────────────────────────────────┤
│          │  Tab1 | Tab2 | Tab3   (MDI)               │
│ Outlook  │───────────────────────────────────────────│
│   Bar    │                                           │
│          │  Содержимое экрана                         │
│ (навиг.) │  (renderScreen)                           │
│          │                                           │
├──────────┴───────────────────────────────────────────┤
│  StatusBar — роли, tenant, выход                     │
└──────────────────────────────────────────────────────┘
```

- **OutlookBar** — навигация, клик открывает вкладку
- **Ribbon** — перестраивается при смене активной вкладки
- **MDI** — управление вкладками, каждая вкладка = свой экран
- **Toast** — уведомления (success / error / warning / info)

---

## 8. API клиент

Файл: `src/protocol/api.ts`

```typescript
import { api } from "@/protocol/api";

// Установить JWT после логина
api.setToken(token);

// GET с параметрами
const result = await api.get<Product>("/api/catalog/products", { sku: "BOLT-01" });

// POST с телом
const result = await api.post<{ product_id: string }>("/api/catalog/products", {
  sku: "BOLT-01", name: "Болт", category: "Метизы", unit: "шт"
});

// Результат — discriminated union
if (result.ok) {
  console.log(result.data);      // типизированные данные
} else {
  console.log(result.error);     // строка с ошибкой
  console.log(result.status);    // HTTP статус (0 если сеть)
}
```

---

## 9. Аутентификация

Текущий режим: **Dev Mode** через `/dev/token`.

Форма логина принимает Tenant ID (UUID) + набор ролей → POST /dev/token → JWT.

Сессия хранится в `sessionStorage` (вкладка закрыта = сессия потеряна). JWT передаётся в `Authorization: Bearer` header каждого запроса.

Доступные роли: `admin`, `warehouse_manager`, `warehouse_operator`, `catalog_manager`, `viewer`.

---

## 10. ЗАПРЕЩЕНО — типичные ошибки

### 10.1. НЕ смешивать метаданные и данные

```
❌ НЕПРАВИЛЬНО:
{
  fields: [
    { id: "name", label: "Название", value: "Болт М8" }  // value в метаданных!
  ]
}

✅ ПРАВИЛЬНО:
// ScreenDescriptor (метаданные):
{ fields: [{ id: "name", label: "Название", field_type: "text" }] }

// DataPayload (данные, отдельно):
{ record: { name: "Болт М8" } }
```

**Почему**: метаданные описывают структуру экрана (одинаковую для всех записей), данные — конкретные значения. Смешение делает экраны неиспользуемыми повторно.

### 10.2. НЕ помещать бизнес-логику в UI

```
❌ НЕПРАВИЛЬНО:
// В main.ts или renderer.ts:
if (data.status === "draft") {
  saveButton.disabled = false;
} else {
  saveButton.disabled = true;
}

✅ ПРАВИЛЬНО:
// Сервер решает и присылает в BehaviorState:
{ actions: { save: { enabled: true } } }
// UI только рендерит то, что пришло
```

**Почему**: если UI сам решает "можно или нельзя" — разные клиенты (web, mobile) будут решать по-разному. Источник истины — сервер.

### 10.3. НЕ хардкодить доступность действий

```
❌ НЕПРАВИЛЬНО:
// В renderer.ts:
if (field.id === "doc_number") {
  input.disabled = true;  // "номер всегда readonly"
}

✅ ПРАВИЛЬНО:
// Behavior определяет:
{ fields: { doc_number: { editable: false } } }
// Renderer применяет общим кодом для ВСЕХ полей
```

**Почему**: рендерер не знает бизнес-смысл полей. Он применяет BehaviorState одинаково ко всем полям.

### 10.4. НЕ создавать per-BC код в UI

```
❌ НЕПРАВИЛЬНО:
ui/src/warehouse/receipt-form.ts    // отдельный файл для склада
ui/src/catalog/product-form.ts      // отдельный файл для каталога

✅ ПРАВИЛЬНО:
// Один рендерер для всех экранов:
ui/src/engine/renderer.ts
// Экраны описаны метаданными:
ui/src/protocol/mock.ts  →  "warehouse.receive": { sections: [...] }
```

**Почему**: движок должен рисовать ЛЮБОЙ экран из метаданных. Если требуется новый файл для нового BC — значит метаданные недостаточно выразительны. Нужно расширять ScreenDescriptor, а не писать специальный код.

### 10.5. НЕ помещать данные в BehaviorState

```
❌ НЕПРАВИЛЬНО:
{ fields: { balance: { visible: true, editable: false, value: "150.00" } } }

✅ ПРАВИЛЬНО:
// BehaviorState:
{ fields: { balance: { visible: true, editable: false, required: false } } }
// DataPayload:
{ record: { balance: "150.00" } }
```

**Почему**: BehaviorState — это маска доступности ("что можно"). DataPayload — это значения ("что показать"). Они меняются по разным причинам и в разное время.

### 10.6. НЕ вычислять навигацию на клиенте

Сейчас `filterNavigation()` — это временный workaround для dev-режима. В production навигация должна приходить с сервера, уже отфильтрованная по ролям и permissions пользователя.

---

## 11. Структура файлов

```
ui/
  src/
    auth/
      login-page.ts       ← форма логина (dev/token)
      session.ts           ← хранение сессии, фильтрация навигации
    engine/
      renderer.ts          ← ScreenDescriptor + Data + Behavior → DOM
    protocol/
      types.ts             ← типы трёх контрактов (TS-зеркало будущих Rust-типов)
      api.ts               ← HTTP клиент с JWT, типизированные API вызовы
      mock.ts              ← определения экранов + behavior (пока в коде, потом из Rust)
    shell/
      outlook-bar.ts       ← левая навигация
      ribbon.ts            ← верхний toolbar
      mdi.ts               ← вкладки (MDI)
      toast.ts             ← уведомления
      icons.ts             ← SVG иконки
    styles/
      app.css              ← Tailwind + тема
    main.ts                ← точка входа: boot → login → shell → actions
  index.html               ← SPA entry point
  package.json             ← зависимости (vite, tailwind, typescript)
  vite.config.ts           ← сборка + proxy на backend
  tsconfig.json            ← TypeScript config
```

---

## 12. Будущее: переход от mock к серверу

Сейчас:
- ScreenDescriptor → захардкожен в `mock.ts`
- BehaviorState → захардкожен в `mock.ts`
- DataPayload → приходит из реального API
- Навигация → захардкожена + фильтрация на клиенте

Целевое состояние:
- ScreenDescriptor → приходит с сервера из `ui_meta` (Rust crate)
- BehaviorState → вычисляется на сервере в `ui_runtime` (Rust crate)
- DataPayload → приходит из API (уже работает)
- Навигация → приходит с сервера, уже отфильтрованная

Для разработчика BC это будет означать: описал экран в Rust → он появился в UI. Без изменения TypeScript-кода.
