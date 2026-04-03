# UI Stack Recommendation: New Web ERP

**Дата**: 2026-04-02
**Контекст**: Backend — Rust modular monolith, HTTP/JSON API, multi-tenant, JWT auth
**Основание**: Функциональная декомпозиция desktop MERP (см. `doc/ui-requirements-analysis.md`)

---

## 1. Выбранный стек

```
┌─────────────────────────────────────────────────┐
│  Deployment: Vite + Docker                      │
├─────────────────────────────────────────────────┤
│  Behavior: XState v5 (state machines)           │
├─────────────────────────────────────────────────┤
│  State: Zustand (client) + TanStack Query (API) │
├─────────────────────────────────────────────────┤
│  Routing: TanStack Router (type-safe)           │
├─────────────────────────────────────────────────┤
│  Forms: React Hook Form + Zod                   │
├─────────────────────────────────────────────────┤
│  Grid: AG Grid Enterprise                       │
├─────────────────────────────────────────────────┤
│  UI Kit: shadcn/ui (Radix + Tailwind)           │
├─────────────────────────────────────────────────┤
│  Framework: React 19 + TypeScript 5.x           │
└─────────────────────────────────────────────────┘
```

---

## 2. Обоснование каждого выбора

### 2.1. React 19 + TypeScript (а не Vue)

Vue 3 — отличный фреймворк, но для ERP с профилем давления MERP:

| Фактор | Vue | React |
|--------|-----|-------|
| AG Grid integration | Community wrapper | First-class adapter от AG Grid team |
| Enterprise form libraries | VeeValidate — хорош, ecosystem меньше | React Hook Form — 40K+ stars, battle-tested в enterprise |
| Component libraries для ERP | PrimeVue, Quasar — хороши для CRUD, слабы в heavy grid | Множество enterprise-grade решений |
| Hiring / community | Меньше enterprise-разработчиков | Проще найти людей с ERP/enterprise опытом |
| XState integration | Есть, но менее зрелый | @xstate/react — first-class |
| Keep-alive tabs (MDI) | Встроенный `<KeepAlive>` — плюс Vue | Решается через state management |

Vue выигрывает в DX для небольших проектов. React выигрывает в ecosystem для enterprise-тяжёлых проектов.

### 2.2. AG Grid Enterprise — 50% успеха проекта

Из анализа: 100+ гридов с inline editing, 10+ типами ячеек, drag-drop, виртуализацией, keyboard navigation.

AG Grid покрывает из коробки:
- Inline editing с 10+ типами ячеек
- Virtual scrolling на 100K+ строк
- Server-Side Row Model (пагинация на бэке)
- Tree Data (TreeView из MERP)
- Clipboard, drag-drop, grouping, column pinning
- Keyboard navigation (Tab/Enter/Escape)
- Цена: ~$1500/dev/year — окупается за первую неделю

**Альтернатива**: TanStack Table (бесплатный, headless) — inline editing, tree data и виртуализацию писать самим. Для 100+ гридов это месяцы работы.

### 2.3. TanStack Query

Кэширование, refetch, optimistic updates, background sync. Идеальная прослойка между React и Rust API. 296 API endpoints.

### 2.4. TanStack Router

Type-safe routing. Каждый route = TypeScript type. Параметры фильтров в URL, search params типизированы. Для ERP с 60+ routes — необходимость.

### 2.5. React Hook Form + Zod

Для 214 CRUD-форм:
- Schema-driven validation (Zod schema = и frontend и backend validation)
- Dirty tracking из коробки
- Field arrays (строки документа в форме)
- Performance: uncontrolled inputs, минимум ререндеров
- Zod schemas можно шарить с Rust backend (через codegen)

### 2.6. Zustand

Минимальный state manager для глобального состояния: active tabs, user session, tenant context, notification queue. Без boilerplate Redux.

### 2.7. shadcn/ui (Radix + Tailwind)

Не библиотека, а набор копируемых компонентов. Полный контроль, нет vendor lock-in. Компоненты: Dialog, Dropdown, Tabs, Toast, Sheet (drawer), Command palette, DatePicker.

### 2.8. XState v5 — управление поведением

См. раздел 4 — полное описание архитектуры state machines.

---

## 3. Как стек решает проблемы из анализа

| Проблема из анализа | Решение |
|---------------------|---------|
| 100+ гридов с inline editing | AG Grid Enterprise — конфигурация, не код |
| 214 CRUD-форм | React Hook Form + Zod schema → генерация форм из API schema |
| Tree + Grid hybrid (DocApprove) | AG Grid Tree Data mode |
| MDI tabs (5-15 вкладок) | TanStack Router + Zustand tab manager + lazy loading |
| Workflow/status transitions | XState finite state machines + backend-driven available actions |
| Field-level permissions | React Hook Form + permission context → auto-disable fields |
| Lookup selectors (28+) | shadcn/ui Combobox + TanStack Query async search |
| Approval routing (Concord) | XState child actor + custom UI на shadcn/ui |
| File attachments | shadcn/ui + native File API + TanStack Query mutation |
| Print/Export | Server-side PDF (Rust) + client preview в iframe |
| Real-time notifications | WebSocket → Zustand store → Toast (shadcn/ui Sonner) |
| Gantt chart | DHTMLX Gantt или Bryntum (framework-agnostic library) |
| Barcode scanner | Native keyboard events (USB scanners = keyboard input) |
| Batch operations | AG Grid checkbox selection + TanStack Query mutation with progress |

---

## 4. UI Logic Architecture: XState State Machines

### 4.1. Проблема

В desktop MERP логика UI размазана:
- Статусы документа — enum в БД + `if/else` в C++
- Доступность кнопок — `m_bCanEdit && status == DRAFT` в 100 местах
- Side effects — `ScopedTransaction` + ручной `UpdateAllViews()`
- Валидация — `GetBusinessObjectErrors()` + per-field checks

В web ERP нужна **единая модель**: состояние → доступные действия → переходы → side effects.

### 4.2. Три слоя данных

```
┌─────────────────────────────────────────────────────────────┐
│  Zustand ─── хранит данные (user, tenant, cache)           │
│  TanStack Query ─── хранит серверные данные (API cache)    │
│  XState ─── управляет поведением (что можно, что нельзя)   │
│                                                             │
│  Это три разных слоя, не конкуренты                        │
└─────────────────────────────────────────────────────────────┘
```

### 4.3. Пример: MoveMaterial (перемещение материалов)

#### Без state machine (как сейчас в MERP):

```cpp
// Размазано по 15 методам в MERP_ViewFormMoveMaterial.cpp
void OnExecute() {
    if (m_bCanEdit && m_iStatus == 1 && !m_bIsDeleted && HasPrivilege("Execute")) {
        // ...
    }
}
void OnDelete() {
    if (m_bCanDelete && m_iStatus == 1 && !m_bIsDeleted && HasPrivilege("Delete")) {
        // ...
    }
}
void OnUnexecute() {
    if (m_bCanEdit && m_iStatus == 4 && HasPrivilege("Unexecute") && !HasDependentDocs()) {
        // ...
    }
}
// + ещё 10 методов с похожими проверками
// Баги: забыл проверку → кнопка доступна когда не должна
```

#### С XState:

```typescript
const moveMaterialMachine = setup({
  types: {
    context: {} as {
      documentId: number | null;
      lines: MoveMaterialLine[];
      permissions: Permissions;
      hasUnsavedChanges: boolean;
    },
    events: {} as
      | { type: 'SAVE' }
      | { type: 'DELETE' }
      | { type: 'EXECUTE' }
      | { type: 'UNEXECUTE' }
      | { type: 'EDIT_LINE'; lineId: number; field: string; value: unknown }
      | { type: 'ADD_LINE' }
      | { type: 'REMOVE_LINE'; lineId: number },
  },
  guards: {
    canEdit:    ({ context }) => context.permissions.canEdit,
    canDelete:  ({ context }) => context.permissions.canDelete,
    canExecute: ({ context }) => context.permissions.canExecute,
    hasLines:   ({ context }) => context.lines.length > 0,
    isValid:    ({ context }) => validateDocument(context).length === 0,
  },
  actors: {
    saveDocument:    fromPromise(({ input }) => api.moveMaterial.save(input)),
    executeDocument: fromPromise(({ input }) => api.moveMaterial.execute(input)),
    deleteDocument:  fromPromise(({ input }) => api.moveMaterial.delete(input)),
    loadDocument:    fromPromise(({ input }) => api.moveMaterial.get(input)),
  },
}).createMachine({
  id: 'moveMaterial',
  initial: 'loading',

  states: {
    // ═══ Загрузка ═══
    loading: {
      invoke: {
        src: 'loadDocument',
        onDone: [
          { guard: 'isExecuted', target: 'executed', actions: 'assignData' },
          { target: 'draft', actions: 'assignData' },
        ],
        onError: 'error',
      },
    },

    // ═══ Черновик ═══
    draft: {
      on: {
        SAVE: {
          guard: and(['canEdit', 'isValid']),
          target: 'saving',
        },
        DELETE: {
          guard: 'canDelete',
          target: 'confirmDelete',
        },
        EXECUTE: {
          guard: and(['canExecute', 'hasLines', 'isValid']),
          target: 'executing',
        },
        EDIT_LINE: {
          guard: 'canEdit',
          actions: 'updateLine',
        },
        ADD_LINE: {
          guard: 'canEdit',
          actions: 'addLine',
        },
        REMOVE_LINE: {
          guard: 'canEdit',
          actions: 'removeLine',
        },
      },
    },

    // ═══ Проведён ═══
    executed: {
      on: {
        UNEXECUTE: {
          guard: 'canExecute',
          target: 'unexecuting',
        },
        // EDIT_LINE — нет в списке = невозможен
        // DELETE — нет = невозможен
        // Машина просто не примет событие
      },
    },

    // ═══ Async operations ═══
    saving: {
      invoke: {
        src: 'saveDocument',
        onDone:  { target: 'draft', actions: 'assignSaved' },
        onError: { target: 'draft', actions: 'showError' },
      },
    },
    executing: {
      invoke: {
        src: 'executeDocument',
        onDone:  { target: 'executed', actions: ['assignExecuted', 'notifySuccess'] },
        onError: { target: 'draft', actions: 'showError' },
      },
    },
    unexecuting: {
      invoke: {
        src: 'unexecuteDocument',
        onDone:  { target: 'draft', actions: 'assignUnexecuted' },
        onError: { target: 'executed', actions: 'showError' },
      },
    },
    confirmDelete: {
      on: {
        CONFIRM: 'deleting',
        CANCEL:  'draft',
      },
    },
    deleting: {
      invoke: {
        src: 'deleteDocument',
        onDone:  { actions: 'navigateToList' },
        onError: { target: 'draft', actions: 'showError' },
      },
    },
    error: {
      on: { RETRY: 'loading' },
    },
  },
});
```

### 4.4. Toolbar автоматически знает что доступно

```typescript
function DocumentToolbar() {
  const actorRef = useDocumentMachine();
  const snapshot = useSelector(actorRef, (s) => s);

  // can() проверяет: текущее состояние + guards (permissions + validation)
  // Одна строка вместо 5 проверок
  return (
    <Toolbar>
      <Button disabled={!snapshot.can({ type: 'SAVE' })}
              onClick={() => actorRef.send({ type: 'SAVE' })}>
        Сохранить
      </Button>
      <Button disabled={!snapshot.can({ type: 'EXECUTE' })}
              onClick={() => actorRef.send({ type: 'EXECUTE' })}>
        Провести
      </Button>
      <Button disabled={!snapshot.can({ type: 'DELETE' })}
              onClick={() => actorRef.send({ type: 'DELETE' })}>
        Удалить
      </Button>
      <Button disabled={!snapshot.can({ type: 'UNEXECUTE' })}
              visible={snapshot.matches('executed')}
              onClick={() => actorRef.send({ type: 'UNEXECUTE' })}>
        Отменить проведение
      </Button>

      {snapshot.matches('saving') && <Spinner />}
    </Toolbar>
  );
}
```

### 4.5. Side effects — в машине, не в компонентах

```typescript
actions: {
  notifySuccess: () => toast.success('Документ проведён'),
  navigateToList: () => router.navigate({ to: '/move-materials' }),
  showError: (_, { error }) => toast.error(error.message),
  updateLine: assign({
    lines: ({ context, event }) =>
      context.lines.map(l =>
        l.id === event.lineId ? { ...l, [event.field]: event.value } : l
      ),
    hasUnsavedChanges: true,
  }),
}
```

### 4.6. Невозможные состояния — невозможны

```
draft → EXECUTE → executing → onDone → executed
                                        ↓
                               EDIT_LINE? → ОТКЛОНЕНО (нет перехода)
                               DELETE?    → ОТКЛОНЕНО (нет перехода)
                               UNEXECUTE  → unexecuting → draft
```

Не нужно помнить "а что если пользователь нажмёт Delete пока документ проведён?" — машина просто не примет событие.

### 4.7. Визуализация и отладка

XState имеет Stately Studio (stately.ai) — визуальный редактор:
- Нарисовать машину визуально → экспортировать код
- Написать код → увидеть диаграмму
- Показать бизнес-аналитику для валидации workflow

### 4.8. Фабрика для всех типов документов

```typescript
// Базовая фабрика — общий скелет для всех документов
function createDocumentMachine<TDoc, TLine>(config: {
  name: string;
  api: DocumentApi<TDoc, TLine>;
  states: DocumentStates;         // draft/executed или 5-step workflow
  guards?: Record<string, Guard>; // доп. проверки
  actions?: Record<string, Action>; // доп. side effects
}) {
  // Возвращает machine с общими паттернами:
  // loading, saving, deleting, confirmDelete, error
  // + кастомные состояния из config.states
}

// Использование — 20 строк на модуль вместо 500
const moveMaterialMachine = createDocumentMachine({
  name: 'moveMaterial',
  api: moveMaterialApi,
  states: simpleTwoStateWorkflow,  // draft ↔ executed
});

const qualityControlMachine = createDocumentMachine({
  name: 'qualityControl',
  api: qualityControlApi,
  states: fiveStepWorkflow,  // create → onExec → exec/eliminated → accepted
  guards: {
    canEliminate: ({ context }) => context.data.defectsResolved,
  },
});
```

### 4.9. Concord (согласование) — вложенная машина

```typescript
const concordMachine = setup({
  // ...
}).createMachine({
  id: 'concord',
  initial: 'idle',
  states: {
    idle: {
      on: { START_APPROVAL: 'selectingRoute' },
    },
    selectingRoute: {
      on: {
        ROUTE_SELECTED: { target: 'registered', actions: 'registerOnRoute' },
        CANCEL: 'idle',
      },
    },
    registered: {
      on: { STEP_ACTION: 'processingStep' },
    },
    processingStep: {
      invoke: {
        src: 'processStep',
        onDone: [
          { guard: 'allStepsApproved', target: 'approved' },
          { guard: 'stepRejected', target: 'rejected' },
          { target: 'registered' }, // ещё есть шаги
        ],
      },
    },
    approved: { type: 'final' },
    rejected: {
      on: { RESTART: 'selectingRoute' },
    },
  },
});

// Document machine включает Concord как child actor:
// draft.on.START_CONCORD → spawns concordMachine
```

### 4.10. Grid inline editing — тоже через машину

```typescript
const gridEditMachine = setup({
  // ...
}).createMachine({
  id: 'gridEdit',
  initial: 'viewing',
  states: {
    viewing: {
      on: {
        CELL_CLICK:   'editing',
        ROW_DBLCLICK: 'openingCard',
        DELETE_ROW:   { guard: 'canDelete', target: 'confirmDeleteRow' },
      },
    },
    editing: {
      on: {
        CELL_CHANGE: { actions: 'updateCellValue' },
        TAB:         { actions: 'moveToNextCell' },
        ENTER:       { actions: 'commitAndMoveDown' },
        ESCAPE:      { target: 'viewing', actions: 'revertCell' },
      },
    },
    // ...
  },
});
```

---

## 5. Итоговая архитектура слоёв

```
┌──────────────────────────────────────────────────────────────┐
│                        UI Components                         │
│  (React + shadcn/ui + AG Grid)                              │
│  Рендерят snapshot машины. Отправляют events.                │
│  Никакой бизнес-логики.                                      │
├──────────────────────────────────────────────────────────────┤
│                     XState Machines                           │
│  Документы: draft ↔ executed, 5-step workflows              │
│  Concord: approval routing                                   │
│  Grid: viewing ↔ editing ↔ saving                           │
│  Shell: auth → loading → ready → sessionExpired             │
│  Модалы: closed → open → submitting → closed                │
│                                                              │
│  Guards = permissions + validation                           │
│  Actions = side effects (toast, navigate, assign)            │
│  Actors = async operations (API calls)                       │
├──────────────────────────────────────────────────────────────┤
│                     Data Layer                               │
│  Zustand: user, tenant, theme, tab manager                  │
│  TanStack Query: API cache, optimistic updates              │
├──────────────────────────────────────────────────────────────┤
│                     Rust Backend API                          │
│  HTTP/JSON endpoints                                         │
│  Server-side validation (Zod schemas shared)                 │
│  Document state transitions (source of truth)                │
└──────────────────────────────────────────────────────────────┘
```

**Ключевой принцип**: XState — **не замена** серверной валидации. Backend остаётся source of truth. XState на клиенте — это **оптимистичная проекция** серверных правил, чтобы UI мгновенно показывал что доступно, не дожидаясь round-trip.

---

## 6. Метрики эффективности

| Метрика | Без state machine | С XState |
|---------|-------------------|----------|
| Код на 1 форму документа | ~500 строк if/else | ~80 строк machine config |
| Баги "кнопка активна когда не должна" | Постоянно | Невозможно by design |
| Добавить новый статус | Найти и обновить 15 методов | Добавить 1 state в машину |
| Тестирование | E2E (медленно, хрупко) | Model-based testing (быстро, полное покрытие) |
| Документация workflow | Текстовое описание | Автогенерируемая диаграмма |
| Onboarding нового разработчика | "Прочитай 3000 строк" | "Посмотри диаграмму" |

---

## 7. Архитектурные паттерны

### 7.1. Schema-driven forms

```typescript
// Zod schema = single source of truth
const MoveMaterialSchema = z.object({
  number: z.string().min(1, "Обязательное поле"),
  date: z.date(),
  storageFromId: z.number({ required_error: "Выберите склад" }),
  storageToId: z.number(),
  status: z.enum(["draft", "executed"]),
  lines: z.array(MoveMaterialLineSchema),
});
// Генерируется из Rust API (OpenAPI → Zod)
// React Hook Form подхватывает автоматически
```

### 7.2. Permission-aware components

```typescript
const { canEdit, canDelete, canExecute } = usePermissions("MoveMaterial");

<FormField disabled={!canEdit} ... />
<Button disabled={!canExecute}>Провести</Button>
// AG Grid: editable: (params) => canEdit && params.data.status === "draft"
```

### 7.3. Reusable Lookup

```typescript
// Один компонент для всех 28+ справочников
<EntityLookup
  entity="contragent"
  value={field.value}
  onChange={field.onChange}
  filters={{ isActive: true }}
  displayField="name"
/>
// Внутри: shadcn Combobox + TanStack Query + debounced search
```

### 7.4. Grid config instead of code

```typescript
const columnDefs: ColDef[] = [
  { field: "number", headerName: "Номер", pinned: "left" },
  { field: "date", headerName: "Дата", cellEditor: "agDateEditor" },
  { field: "nomenclatura", headerName: "Номенклатура",
    cellEditor: EntityLookupEditor,
    cellEditorParams: { entity: "nomenclatura" } },
  { field: "quantity", headerName: "Кол-во", editable: true, type: "numericColumn" },
  { field: "price", headerName: "Цена", editable: true, type: "numericColumn" },
  { field: "total", headerName: "Итого",
    valueGetter: (p) => p.data.quantity * p.data.price },
];
```

---

## 8. Что НЕ брать

| Технология | Почему нет |
|------------|-----------|
| Next.js / Nuxt | Rust backend, SSR/SSG не нужен для ERP. SPA + API = правильная архитектура |
| Redux / Pinia | Overkill. Zustand покрывает всё без boilerplate |
| Material UI (MUI) | Тяжёлый, opinionated, сложно кастомизировать |
| Ant Design | Chinese-first ecosystem, тяжёлый bundle, сложно кастомизировать |
| Handsontable | Дорогой, Excel-like (не ERP-like), хуже AG Grid для enterprise |
| Custom grid | **Никогда**. 6-12 месяцев работы, которые AG Grid уже сделал |

---

## 9. Roadmap внедрения

### Phase 1 (месяц 1-2): Shell + Auth + первый модуль
- React + Vite + TanStack Router
- Auth (JWT) + tenant context
- Navigation sidebar
- AG Grid: первый list view
- React Hook Form: первая CRUD-форма
- XState: первая document machine
- Один модуль полностью (например, Справочник номенклатуры — tree + grid + CRUD)

### Phase 2 (месяц 3-4): Документы + Workflow
- Document workflow engine (status transitions) на XState
- MoveMaterial или InOrder полностью (list → card → lines grid → execute)
- File attachments
- Print/Export (server PDF)

### Phase 3 (месяц 5-6): Согласование + Dashboard
- Concord (approval routing) — XState child actor
- Personal Task List
- Notifications (WebSocket)
- Dashboard widgets

### Phase 4 (месяц 7+): Остальные модули
- По 2-3 модуля в месяц (паттерны уже отработаны)
- Gantt (if needed)
- Specialized views

---

## 10. Ключевые выводы

1. **AG Grid Enterprise** решает 50% проблемы — inline editing, virtualization, tree data, keyboard. Без него — полгода на самописный grid.

2. **XState** решает управление поведением — невозможные состояния невозможны, toolbar автоматически знает что доступно, side effects централизованы.

3. **React Hook Form + Zod** решает 30% проблемы — 214 форм через schema, не через императивный код.

4. **TanStack Query** решает кэширование и синхронизацию — 296 API endpoints, optimistic updates, background refetch.

5. **shadcn/ui** даёт UI-kit без vendor lock-in — копируешь компоненты, полный контроль.

6. **Всё типизировано** — TypeScript end-to-end, от Rust API до React component.

**"Это просто интерфейс"** — да, но интерфейс с 800+ компонентами, 100+ гридами и 214 формами. С правильным стеком это конфигурация. С неправильным — вечная стройка.
