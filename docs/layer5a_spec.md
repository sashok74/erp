# Layer 5a — BC Runtime: трейты + Command Pipeline
> Подробное ТЗ | ERP Pilot on Rust
> Дата: 2026-03-23 | Привязка: ADR v1, BCR (Command Pipeline diagram)
> Предусловие: Layer 1 (Kernel) + Layer 3a (Event Bus) выполнены

---

## Зачем этот слой

Runtime — **конвейер обработки команд**. Это сердце системы: каждый запрос на изменение состояния проходит через один и тот же pipeline. Разработчик BC пишет только CommandHandler (бизнес-логика). Всё остальное — авторизация, хуки, транзакция, аудит — делает runtime.

```
HTTP request
  → Auth check           ← runtime
  → Extension hook       ← runtime
  → BEGIN TX             ← runtime
  → CommandHandler       ← разработчик BC пишет ТОЛЬКО ЭТО
  → Domain events        ← runtime
  → Outbox               ← runtime
  → COMMIT               ← runtime
  → After-commit hook    ← runtime
  → Audit log            ← runtime
  → Response
```

### Почему сейчас, а не после БД

Pipeline зависит от **трейтов**, не от PostgreSQL. Auth — trait. Audit — trait. EventBus — trait (уже есть из Layer 3a). UnitOfWork — trait. Мы определяем контракты и собираем Pipeline со stub-реализациями. Тестируем без БД, за миллисекунды.

Когда придёт Layer 2 (PostgreSQL + Clorinde), мы подставим реальные реализации вместо stubs. Pipeline не изменится — только конфигурация в точке входа.

---

## Что мы изучим в этом слое (Rust)

| Концепция | Где применяется | Зачем в Rust |
|-----------|----------------|--------------|
| Composition over inheritance | Pipeline из набора traits | В Rust нет наследования — composition единственный путь |
| `Arc<dyn Trait>` | Pipeline хранит зависимости как trait objects | Dependency injection через shared ownership |
| Async closures / spawn | after_command hooks | Fire-and-forget async side effects |
| Generic methods | `pipeline.execute::<H>()` | Один метод для любого CommandHandler |
| Trait с associated types | `CommandHandler::Cmd`, `CommandHandler::Result` | Type-safe связь команда ↔ результат |
| `#[async_trait]` | Все handler traits | Async + dyn compatibility |
| Error propagation | `?` operator через цепочку вызовов | DomainError → AppError → HTTP response |
| Unit testing с mocks | Stub-реализации каждого trait | Тестирование без инфраструктуры |

---

## Структура файлов

```
crates/runtime/src/
├── lib.rs                  ← pub mod + re-exports
├── command_handler.rs      ← CommandHandler trait
├── query_handler.rs        ← QueryHandler trait
├── pipeline.rs             ← CommandPipeline (полный конвейер)
├── module.rs               ← BoundedContextModule trait
├── ports.rs                ← трейты-порты: PermissionChecker, AuditLog, ExtensionHooks, UnitOfWork
└── stubs.rs                ← Stub-реализации для тестирования
```

---

## Задача 5a.1 — Порты: трейты зависимостей Pipeline

### Зачем в ERP

Pipeline вызывает: авторизацию, хуки, аудит, Unit of Work. Каждый — trait (порт). Реализации придут позже:

| Порт (trait) | Реализация | Когда |
|-------------|-----------|-------|
| PermissionChecker | RBAC + JWT | Layer 4a |
| AuditLog | PostgreSQL writer | Layer 4b |
| ExtensionHooks | Lua/WASM sandbox | Layer 8 |
| UnitOfWorkFactory | PostgreSQL TX + RLS | Layer 2 |

Сейчас — stubs: NoopPermissionChecker (всё разрешено), NoopAuditLog, NoopExtensionHooks, InMemoryUnitOfWork.

### Зачем в Rust (что учим)

**Dependency Inversion** — Pipeline зависит от абстракций (traits), не от PostgreSQL/JWT/Lua. В Rust это не «хорошая практика», а единственный способ избежать циклических зависимостей между crate'ами.

### Требования к коду

**Файл: `crates/runtime/src/ports.rs`**

Трейты:

1. **PermissionChecker** — `check_permission(ctx, command_name) → Result<(), AppError>`
2. **AuditLog** — `log(ctx, command_name, result: &serde_json::Value)`
3. **ExtensionHooks** — `before_command(name, ctx) → Result<(), AppError>` + `after_command(name, ctx) → Result<(), anyhow::Error>`
4. **UnitOfWorkFactory** — `begin(ctx) → Result<UoW, AppError>` (associated type `type UoW: UnitOfWork`)
5. **UnitOfWork** — `add_outbox_entry(EventEnvelope)` + `commit(self)` + `rollback(self)`

Все: `#[async_trait]`, bounds `Send + Sync + 'static`.

### Тесты

- Каждый trait object-safe: `Arc<dyn PermissionChecker>` компилируется

---

## Задача 5a.2 — Stubs для тестирования

### Зачем в ERP

Без stubs нельзя тестировать Pipeline без БД. Stubs — минимальные реализации: всё разрешено, ничего не пишет, in-memory UoW.

### Зачем в Rust (что учим)

**Testing через trait substitution** — вместо mock-фреймворков. Hand-written stubs проще и прозрачнее. Компилятор проверяет всё.

### Требования к коду

**Файл: `crates/runtime/src/stubs.rs`**

- `NoopPermissionChecker` — check_permission → Ok(())
- `NoopAuditLog` — log → ничего
- `NoopExtensionHooks` — before/after → Ok(())
- `InMemoryUnitOfWorkFactory` → создаёт `InMemoryUnitOfWork`
- `InMemoryUnitOfWork` — add_outbox_entry добавляет в Vec, commit устанавливает флаг, rollback ничего

Для тестов: stubs с `Arc<AtomicBool>` или `Arc<Mutex<Vec<String>>>` для записи вызовов — **SpyPermissionChecker**, **SpyAuditLog**.

### Тесты

- NoopPermissionChecker всегда Ok
- InMemoryUnitOfWork: add_outbox_entry → entries.len() растёт
- InMemoryUnitOfWork: commit → committed flag

---

## Задача 5a.3 — CommandHandler trait

### Зачем в ERP

CommandHandler — **единственное, что пишет разработчик BC**. Handler получает команду, RequestContext и UnitOfWork, возвращает результат.

### Зачем в Rust (что учим)

**Associated types** — `type Cmd: Command`, `type Result: Serialize + Send`. Handler жёстко связан со своей командой. ReceiveGoodsHandler нельзя вызвать с ShipGoodsCommand — compile error.

### Требования к коду

**Файл: `crates/runtime/src/command_handler.rs`**

```rust
#[async_trait]
pub trait CommandHandler: Send + Sync + 'static {
    type Cmd: Command;
    type Result: Serialize + Send;

    async fn handle(
        &self,
        cmd: &Self::Cmd,
        ctx: &RequestContext,
        uow: &mut dyn UnitOfWork,
    ) -> Result<Self::Result, AppError>;
}
```

### Тесты

- EchoHandler: принимает EchoCommand, возвращает EchoResult

---

## Задача 5a.4 — QueryHandler trait

### Зачем в ERP

Query (CQRS) — чтение. Не мутирует, не создаёт событий, не требует транзакции. Queries можно кэшировать, маршрутизировать на read-реплику.

### Требования к коду

**Файл: `crates/runtime/src/query_handler.rs`**

```rust
#[async_trait]
pub trait QueryHandler: Send + Sync + 'static {
    type Query: Send + Sync;
    type Result: Serialize + Send;

    async fn handle(
        &self,
        query: &Self::Query,
        ctx: &RequestContext,
    ) -> Result<Self::Result, AppError>;
}
```

### Тесты

- PingQueryHandler → PongResult

---

## Задача 5a.5 — CommandPipeline: полный конвейер

### Зачем в ERP

Центральный компонент. Собирает все шаги SVG-диаграммы: auth → hooks → tx → handler → commit → audit.

### Зачем в Rust (что учим)

**Composition** — Pipeline содержит `Arc<dyn Trait>` для каждой зависимости. Это dependency injection в Rust.

**Generic method** — `execute<H: CommandHandler>()` параметризован handler'ом. Одна функция для любого handler'а с type-safe Cmd/Result.

**`tokio::spawn`** — after_command hooks: fire-and-forget после COMMIT.

### Требования к коду

**Файл: `crates/runtime/src/pipeline.rs`**

```rust
pub struct CommandPipeline<UF: UnitOfWorkFactory> {
    uow_factory: Arc<UF>,
    bus: Arc<dyn EventBus>,
    auth: Arc<dyn PermissionChecker>,
    extensions: Arc<dyn ExtensionHooks>,
    audit: Arc<dyn AuditLog>,
}
```

Метод `execute<H: CommandHandler>()`:

1. `auth.check_permission()` → Err = прерывание
2. `extensions.before_command()` → Err = прерывание
3. `uow_factory.begin(ctx)` → BEGIN
4. `handler.handle(cmd, ctx, &mut uow)` → Err = rollback
5. `uow.commit()` → COMMIT
6. `tokio::spawn(extensions.after_command())` → fire-and-forget
7. `audit.log()` → запись
8. Return result

### Тесты (все с stubs, без БД)

- **Happy path**: execute EchoHandler → Ok, result correct
- **Auth reject**: PermissionChecker returns Err → handler NOT called
- **Hook reject**: before_command returns Err → handler NOT called
- **Handler error**: handler returns Err → rollback called, commit NOT called
- **Commit**: handler Ok → commit called, rollback NOT called

Использовать Spy-stubs с `Arc<AtomicBool>` для проверки вызовов.

---

## Задача 5a.6 — BoundedContextModule trait

### Зачем в ERP

Gateway не знает конкретные BC. Каждый BC регистрируется как модуль: отдаёт routes и подписывает handler'ы на шину.

### Требования к коду

**Файл: `crates/runtime/src/module.rs`**

```rust
#[async_trait]
pub trait BoundedContextModule: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn routes(&self) -> axum::Router;
    async fn register_handlers(&self, bus: &dyn EventBus);
}
```

### Тесты

- Object-safe: `Box<dyn BoundedContextModule>` компилируется

---

## Задача 5a.7 — Финальная сборка

**lib.rs**: pub mod для всех 6 модулей + re-exports.

**Cargo.toml**: kernel, event_bus, tokio, serde, serde_json, async-trait, anyhow, tracing, axum.

**Проверка:**
```bash
cargo build --workspace
cargo test -p runtime
cargo clippy -p runtime -- -D warnings
just check
```

---

## Сводка

| Файл | Содержание |
|------|-----------|
| ports.rs | PermissionChecker, AuditLog, ExtensionHooks, UnitOfWork traits |
| stubs.rs | Noop + Spy stubs |
| command_handler.rs | CommandHandler trait |
| query_handler.rs | QueryHandler trait |
| pipeline.rs | CommandPipeline (9 шагов конвейера) |
| module.rs | BoundedContextModule trait |

### Чему научились

Composition, `Arc<dyn Trait>`, generic methods, async trait, `tokio::spawn`, error propagation, testing с stubs.

### Следующий шаг

Layer 5a готов → **Layer 4a (Auth)**: JWT issue/verify, RBAC PermissionChecker (реализация порта из ports.rs). Без БД.
