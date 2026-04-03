# Review: UI Architecture — Metadata-Driven ERP, Rust-First

**Дата**: 2026-04-03
**Рецензируемый документ**: `doc/ui-architecture.md` (UI Architecture: Metadata-Driven ERP, Rust-First)
**Контекст**: Сравнение с `doc/ui-stack-recommendation.md` и `doc/ui-requirements-analysis.md`

---

## 1. Что документ делает правильно

### 1.1. Metadata-driven — верная цель

Описывать экраны метаданными, а не ручным UI-кодом — правильная архитектура для ERP с 60+ однотипными формами. Фабрика `createDocumentMachine` из stack-recommendation и `ScreenMeta` из architecture решают одну задачу разными средствами.

### 1.2. Server as single source of truth — абсолютно

Stack-recommendation тоже это утверждает ("XState — оптимистичная проекция серверных правил"). Architecture идёт дальше: клиент вообще не вычисляет доступность, а получает `ResolvedAction.enabled = true/false`.

### 1.3. Primitive reduction (6 примитивов) — отличная абстракция

Сжатие 800+ компонентов до Workspace / Collection / Record / Selector / ActionSurface / Feedback — правильный уровень мышления для архитектуры.

### 1.4. Staged adoption — разумно

Не тащить React/Vue/AG Grid пока не доказана необходимость.

---

## 2. Риски

### 2.1. Askama + HTML tables для Collection — упрётся быстро

Документ сам это предвидит (Phase 3: "если пилот уперся в возможности HTML table"). Но из анализа MERP ясно, что это не "если", а "когда" — и очень скоро:

- **Inline editing** (10+ типов ячеек) — первый же складской документ (MoveMaterial) требует редактирование строк прямо в таблице. HTML `<table>` + server round-trip на каждое изменение ячейки — это 200-500ms latency на каждый Tab. Кладовщик вводит 50 строк за раз.
- **Virtual scroll** — складские остатки, номенклатура — тысячи строк. Server-rendered HTML table с 5000 `<tr>` — тяжело.
- **Keyboard navigation** (Tab/Enter/Escape между ячейками) — это client-side по определению. Сервер не может управлять фокусом.

**Открытый вопрос**: как `ui_runtime` решает inline editing в Collection? Каждое нажатие Tab = HTTP запрос? Или это implicit acknowledgment что для Collection нужен JS?

### 2.2. Разрыв между архитектурой и первым пользователем

Документ предлагает сначала построить `ui_meta` + `ui_runtime` (Phase 0-1), и только потом первый реальный экран (Phase 2-4):

- `ui_meta` + `ui_runtime` — это фактически **свой UI framework на Rust**. Это 2-4 месяца на инфраструктуру до первого бизнес-экрана.
- React + AG Grid + React Hook Form — можно показать рабочий склад через 6-8 недель.

Вопрос приоритетов: что важнее — архитектурная чистота или ранний feedback от пользователей?

### 2.3. HTMX — не silver bullet для ERP interaction density

HTMX отлично работает для:
- CRUD страниц
- Фильтрация с server-side render
- Partial updates

HTMX плохо работает для:
- Inline grid editing (Tab между ячейками, undo/redo)
- Drag-drop (между гридами, reorder строк)
- Optimistic UI (показать результат до ответа сервера)
- Complex keyboard flows (Ctrl+S, Escape для отмены)

Для ERP с 80% времени в гриде — HTMX покрывает ~40% сценариев. Остальные 60% всё равно потребуют JS.

### 2.4. "Не переносить UI-логику в клиент" — абсолют, который может навредить

Принцип правильный как default. Но есть сценарии, где client-side logic — не "утечка", а необходимость:

| Сценарий | Server-only? | Почему |
|----------|-------------|--------|
| "Кнопка Execute disabled в draft" | Да, server resolved | Бизнес-правило |
| "Показать итого при вводе quantity × price" | Нет, client | Мгновенный feedback, не ждать round-trip |
| "Tab → следующая ячейка грида" | Нет, client | Focus management |
| "Undo последнее изменение в форме" | Нет, client | Local state |
| "Показать validation error при blur" | Гибрид | Server rules, client timing |
| "Drag строку в другую позицию" | Нет, client | Interaction |

Запрет "не переносить UI-логику в клиент" нужно уточнить: **бизнес-правила доступности** — на сервере, **interaction logic** (focus, undo, drag, calculated fields) — на клиенте.

---

## 3. Сравнение двух подходов

| Аспект | ui-stack-recommendation | ui-architecture |
|--------|------------------------|-----------------|
| Server = source of truth | Да (XState = проекция) | Да (ui_runtime = единственный) |
| Metadata-driven | Частично (schema-driven forms, grid config) | Полностью (ScreenMeta → ResolvedScreen) |
| Минимум примитивов | Нет (каталог компонентов) | Да (6 примитивов) |
| Staged adoption | Нет (весь стек сразу) | Да (Rust → HTMX → AG Grid) |
| Enterprise grid | AG Grid сразу | AG Grid когда доказано |
| UI logic location | Client (XState) | Server (ui_runtime) |
| Time to first screen | 6-8 недель | 3-4 месяца (Phase 0-2) |
| Architectural purity | Средняя | Высокая |
| Risk of over-engineering | Низкий | Средний |
| Risk of under-engineering | Средний | Низкий |

**Ключевое расхождение одно**: где живёт UI state machine — на клиенте (XState) или на сервере (ui_runtime).

---

## 4. Рекомендация: гибрид

Оба документа правы в разных аспектах. Оптимальный путь:

### 4.1. `ui_meta` + `ui_runtime` в Rust — делать

Это правильная архитектура. Сервер определяет доступность, видимость, workflow. `ResolvedScreen` — отличный контракт.

### 4.2. Клиент — не Askama-only

Для Collection с inline editing нужен JS. Вопрос — сколько:

- **Минимальный вариант**: Web Components (без фреймворка) для Collection, остальное server-rendered
- **Средний вариант**: HTMX для навигации/Record + AG Grid для Collection
- **Максимальный вариант**: React SPA потребляет `ResolvedScreen` как JSON API

### 4.3. XState на клиенте — убрать

Если `ui_runtime` на сервере уже вычисляет состояние, дублировать эту логику в XState — нарушение single source of truth. Клиент просто читает `resolved.actions[i].enabled`.

### 4.4. Interaction logic — на клиенте, но минимально

Focus management, undo, drag-drop, calculated fields — это не бизнес-логика, это UX. Им место на клиенте. Без фреймворка или с минимальным (Alpine.js / vanilla TS).

### 4.5. Phase 0 критичен

`ui_meta` + `ui_runtime` + transport format — определяют всё остальное. Пока этого нет, выбор React vs Askama vs HTMX — преждевременная оптимизация.

---

## 5. Уточнение границы server / client logic

### 5.1. На сервере (ui_runtime) — бизнес-поведение

- Доступность действий (enabled/disabled/hidden)
- Видимость секций и полей
- Read-only/editable по статусу и роли
- Workflow transitions (какие переходы возможны)
- Side effects после команд (UiEffect[])
- Валидация бизнес-правил

### 5.2. На клиенте — interaction logic

- Focus management (Tab, Enter, Escape)
- Keyboard shortcuts (Ctrl+S → submit form)
- Undo/redo в рамках текущей сессии
- Calculated fields (quantity × price = total) — мгновенный feedback
- Drag-drop interaction
- Virtual scroll / lazy rendering
- Optimistic UI (показать изменение до ответа сервера)
- Animation / transitions

### 5.3. Правило разграничения

> **Если решение зависит от бизнес-правил, ролей или состояния сущности — сервер.**
> **Если решение зависит от позиции курсора, фокуса или UX-feedback — клиент.**
> **Если и то и другое — сервер определяет правила, клиент определяет timing.**

---

## 6. Открытые вопросы для следующей итерации

1. **Transport format**: JSON API (`ResolvedScreen` как JSON) или HTML fragments (Askama + HTMX)?
2. **Collection inline editing**: на чём строить client-side grid? AG Grid? Custom web component? HTMX-compatible solution?
3. **Validation flow**: сервер возвращает resolved validation rules, клиент применяет их при blur? Или full round-trip на каждый blur?
4. **Calculated fields**: формулы приходят с сервера в metadata (expression engine) или hardcoded на клиенте?
5. **Optimistic UI**: нужен ли? Если да, для каких сценариев? Inline editing? Save? Status transition?
6. **Offline**: если складские/производственные сценарии требуют offline, metadata-driven подход усложняется (нужен client-side resolution).
