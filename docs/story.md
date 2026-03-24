# История: как думает Rust-разработчик, строя ERP

> Внутренний монолог опытного разработчика. Каждый шаг — мысль, подход, результат.

---

## Глава 0. С чего начать

### Мысль

У меня есть задача: ERP для дискретного производства. Modular monolith, PostgreSQL, Event Sourcing. Десяток bounded contexts, multi-tenancy, расширения через Lua/WASM.

Первый порыв — начать с базы данных и складского модуля. Таблицы, запросы, REST API. Видимый результат за неделю. Но я знаю, чем это кончается: через месяц crate'ы завязаны друг на друга, каждое изменение ломает три модуля, тесты требуют запущенный Postgres.

**Ключевое решение: строить изнутри наружу.** Сначала контракты (traits), потом инфраструктура. Сначала порты, потом адаптеры. Это Hexagonal Architecture, но не как академическое упражнение — а потому что я хочу тестировать Pipeline за миллисекунды, без БД.

### Подход: trait-first, inside-out

```
Фаза 1 — Абстракции (без БД, мгновенные тесты):
  kernel → event_bus → runtime → auth

Фаза 2 — Инфраструктура (подключаем PostgreSQL):
  db → clorinde → audit → seq_gen

Фаза 3 — Бизнес-логика (домен):
  warehouse (MVP)
```

Каждый слой компилируется и тестируется до того, как существует следующий. К моменту, когда я напишу первый SQL-запрос — Pipeline, авторизация и шина событий уже полностью протестированы.

### Результат

11 crate'ов в workspace. Зависимости строго в одном направлении. Kernel ни от кого не зависит. Gateway зависит от всех. Каждый crate можно тестировать отдельно: `cargo test -p kernel` за 0.3 секунды.

---

## Глава 1. Kernel — язык, на котором говорит система

### Мысль

Kernel — не «общий код». Kernel — это **словарь**. Если два BC хотят обменяться событием, им нужны общие типы. Но если я положу в kernel бизнес-типы (SKU, Money, Quantity), получится God Object. Каждый BC начнёт тянуть kernel ради чужих типов.

Поэтому в kernel — только инфраструктурные контракты: идентификаторы, ошибки, трейты для команд и событий, формат CloudEvent. **Бизнес-типы (Value Objects) — в domain каждого BC.** BC общаются через integration events с примитивами в payload.

### Подход: Newtype + UUID v7

Голый `Uuid` в сигнатурах функций — приглашение к ошибкам. `fn transfer(from: Uuid, to: Uuid, user: Uuid)` — три UUID, перепутать элементарно. Компилятор не спасёт.

Newtype pattern:

```rust
pub struct TenantId(Uuid);
pub struct UserId(Uuid);
pub struct EntityId(Uuid);
```

Теперь `fn transfer(from: EntityId, to: EntityId, user: UserId)` — компилятор не даст передать UserId вместо EntityId. Zero-cost: в рантайме это тот же Uuid, newtype стирается при компиляции.

UUID v7 вместо v4 — потому что v7 содержит timestamp. Идентификаторы сортируются по времени создания. Для базы данных это означает sequential inserts в B-tree индекс, не random. Для логов — события упорядочены без отдельного поля `created_at`.

### Подход: двухуровневые ошибки через thiserror

```rust
enum DomainError {          // бизнес: "не хватает товара"
    InsufficientStock { required, available },
    NotFound(String),
    ConcurrencyConflict { expected, actual },
    BusinessRule(String),
}

enum AppError {             // приложение: оборачивает бизнес + добавляет инфра
    Domain(#[from] DomainError),  // ← автоматическая конвертация через ?
    Unauthorized(String),
    Validation(String),
    Internal(String),
}
```

Handler пишет `Err(DomainError::NotFound(...))` — `?`-operator автоматически конвертирует в `AppError::Domain(NotFound(...))`. Middleware в Gateway потом маппит: `NotFound → 404`, `InsufficientStock → 422`, `Unauthorized → 401`. Разработчик BC никогда не думает о HTTP-кодах.

`#[from]` — это derive-макрос thiserror. Генерирует `impl From<DomainError> for AppError`. В C++ аналога нет — там обычно коды ошибок или иерархия исключений с ручным перебросом.

### Подход: DomainEvent как trait, CloudEvent как обёртка

```rust
trait DomainEvent: Send + Sync + 'static {
    fn event_type(&self) -> &'static str;    // "warehouse.goods_received.v1"
    fn aggregate_id(&self) -> Uuid;
}

struct CloudEvent<T: DomainEvent> {  // CNCF CloudEvents v1.0
    id: Uuid,
    source: String,
    event_type: String,      // копия из DomainEvent
    data: T,                 // типизированный payload
    // ERP extensions:
    tenant_id: TenantId,
    correlation_id: Uuid,    // сквозная трассировка
    causation_id: Uuid,      // что вызвало это событие
}
```

`DomainEvent` — trait, потому что каждый BC определяет свои события. `CloudEvent<T>` — generic обёртка с метаданными. CloudEvents v1.0 — открытый стандарт, если завтра нужно отправлять события в Kafka или webhook — формат уже стандартный.

`correlation_id` + `causation_id` — для трассировки цепочек: «заказ создал резерв, резерв создал проводку». Без них отладка distributed systems — ад.

### Результат

```
kernel/src/
├── types.rs      — TenantId, UserId, EntityId, RequestContext
├── errors.rs     — DomainError, AppError
├── commands.rs   — Command trait, CommandEnvelope
├── events.rs     — DomainEvent trait, CloudEvent<T>
└── entity.rs     — Entity, AggregateRoot traits
```

6 файлов, ~740 строк, 0 зависимостей от БД. Полный набор контрактов для всей системы.

---

## Глава 2. Event Bus — где type erasure становится необходимостью

### Мысль

Шина событий — центр межмодульной коммуникации. Warehouse публикует `GoodsReceived`, Finance подписывается и создаёт проводку. Проблема: шина не должна знать конкретные типы событий. Она получает «что-то» и доставляет «тем, кому нужно».

В C++ это решается через `void*` или `std::any`. В Rust — через **type erasure**: типизированный handler заворачивается в trait object, а событие сериализуется в `serde_json::Value`.

### Подход: трёхслойная архитектура шины

**Слой 1: типизированный контракт**

```rust
#[async_trait]
trait EventHandler: Send + Sync + 'static {
    type Event: DomainEvent;
    async fn handle(&self, event: &Self::Event) -> Result<(), anyhow::Error>;
    fn handled_event_type(&self) -> &'static str;
}
```

Разработчик BC пишет handler с конкретным типом: `impl EventHandler for FinanceHandler { type Event = GoodsReceived; ... }`. Компилятор проверяет типы.

**Слой 2: type erasure (мост)**

```rust
#[async_trait]
trait ErasedEventHandler: Send + Sync + 'static {
    async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<()>;
    fn event_type(&self) -> &'static str;
}

struct EventHandlerAdapter<H: EventHandler> { handler: H }
```

`EventHandlerAdapter` — обёртка. Она принимает `EventEnvelope` (с `serde_json::Value` payload), десериализует в `H::Event`, вызывает типизированный `handler.handle()`. Граница типизации проходит здесь.

**Слой 3: транспорт (EventEnvelope)**

```rust
struct EventEnvelope {
    event_type: String,           // ключ маршрутизации
    payload: serde_json::Value,   // стёртый тип
    tenant_id, correlation_id, ...
}
```

Шина работает только с `EventEnvelope`. Не знает о `GoodsReceived` или `InvoiceCreated`. Routing по `event_type` строке.

### Почему не enum

Альтернатива — один большой `enum AllEvents { GoodsReceived(...), InvoiceCreated(...), ... }`. Каждый новый BC добавляет вариант → перекомпиляция kernel → все зависимые crate'ы пересобираются. Нарушение Open/Closed Principle. С type erasure — новый BC регистрирует handler, шина не меняется.

### Подход: RwLock для реестра handler'ов

```rust
struct HandlerRegistry {
    handlers: RwLock<HashMap<String, Vec<Arc<dyn ErasedEventHandler>>>>,
}
```

`RwLock`, не `Mutex` — потому что регистрация (write) происходит один раз при старте, а dispatch (read) — на каждое событие. `RwLock` позволяет параллельное чтение. `tokio::sync::RwLock` — потому что `await` внутри критической секции.

### Подход: две стратегии публикации

```rust
trait EventBus {
    async fn publish(&self, envelope: EventEnvelope);         // fire-and-forget
    async fn publish_and_wait(&self, envelope: EventEnvelope); // sync
}
```

`publish` — для after-commit side effects. Не ждём, ошибки handler'ов не ломают основной flow.
`publish_and_wait` — для domain events внутри транзакции. Ждём обработки, ошибка = rollback.

### Результат

```
event_bus/src/
├── traits.rs     — EventHandler, EventBus
├── envelope.rs   — EventEnvelope (type-erased transport)
├── registry.rs   — ErasedEventHandler, EventHandlerAdapter, HandlerRegistry
└── bus.rs        — InProcessBus (tokio channels)
```

Шина полностью работает in-process. При переходе к микросервисам — пишем `RabbitMqBus`, реализующий тот же `EventBus` trait. Ни один BC не меняется.

---

## Глава 3. Pipeline — конвейер, который пишут один раз

### Мысль

Каждая команда в ERP проходит одинаковый путь: проверь права → вызови хуки → открой транзакцию → выполни handler → зафиксируй → запиши аудит. Если разработчик BC будет писать эту последовательность руками — кто-то забудет аудит, кто-то не откатит транзакцию при ошибке.

**Решение: разработчик BC пишет только handler. Всё остальное делает Pipeline.**

### Подход: порты как trait objects

```rust
struct CommandPipeline<UF: UnitOfWorkFactory> {
    uow_factory: Arc<UF>,
    bus:         Arc<dyn EventBus>,
    auth:        Arc<dyn PermissionChecker>,
    extensions:  Arc<dyn ExtensionHooks>,
    audit:       Arc<dyn AuditLog>,
}
```

Пять зависимостей — пять trait objects. Pipeline не знает, кто проверяет права (JWT? LDAP? Noop?), как открывается транзакция (PostgreSQL? In-memory mock?), куда пишется аудит (таблица? stdout? /dev/null?).

`UnitOfWorkFactory` — generic parameter, а не trait object. Потому что `UnitOfWorkFactory::UoW` — associated type, и Pipeline должен знать конкретный тип UoW для `handler.handle(cmd, ctx, &mut uow)`. Trait object не может иметь associated type с разными значениями в runtime.

### Подход: вложенность ошибок диктует порядок шагов

```rust
async fn execute(&self, handler, cmd, ctx) -> Result<H::Result, AppError> {
    // 1. auth — до всего: если нет прав, даже транзакцию не открываем
    self.auth.check_permission(ctx, cmd.command_name()).await?;

    // 2. before_hook — хук может отменить команду (валидация от Lua-плагина)
    self.extensions.before_command(cmd.command_name(), ctx).await?;

    // 3. begin TX — только если auth и hooks прошли
    let mut uow = self.uow_factory.begin(ctx).await?;

    // 4. handler — бизнес-логика
    let result = handler.handle(cmd, ctx, &mut uow).await;

    match result {
        Ok(value) => {
            // 5. commit
            Box::new(uow).commit().await?;
            // 6. after_hook — fire-and-forget (email, webhook)
            tokio::spawn(async move { extensions.after_command(...) });
            // 7. audit
            self.audit.log(ctx, cmd_name, &audit_value).await;
            Ok(value)
        }
        Err(e) => {
            // rollback при ошибке handler'а
            Box::new(uow).rollback().await?;
            Err(e)
        }
    }
}
```

Порядок неслучаен:
- Auth первым — зачем тратить ресурсы на транзакцию, если нет прав?
- Before-hook до TX — хук может отменить, не начиная транзакцию
- After-hook через `tokio::spawn` — fire-and-forget, не блокирует response
- Audit после commit — логируем только успешные операции

### Подход: stubs вместо mocks

```rust
struct NoopPermissionChecker;   // всегда Ok
struct SpyPermissionChecker;    // записывает вызовы + настраиваемый результат
struct InMemoryUnitOfWork;      // Vec вместо PostgreSQL, флаги commit/rollback
```

Не mockito, не mockall — руками. Потому что стабы просты (5-15 строк), а mock-фреймворки добавляют магию и зависимости. `SpyPermissionChecker` — это `AtomicBool called` + `Option<String> deny_reason`. Всё.

`InMemoryUnitOfWorkFactory` прокидывает `Arc<AtomicBool>` для committed/rolled_back — тест может проверить: «при ошибке handler'а pipeline вызвал rollback, не commit».

### Результат

Pipeline тестируется за миллисекунды, без PostgreSQL, без Redis, без HTTP-сервера. Пять тестов покрывают все сценарии:
- Happy path: handler → commit → audit
- Auth reject: handler не вызван, UoW не начат
- Hook reject: то же
- Handler error: rollback, не commit
- Audit записывает имя команды

---

## Глава 4. Auth — первая реальная подстановка

### Мысль

До этого момента Pipeline работал с `NoopPermissionChecker` — всё разрешено. Сейчас мы подставляем настоящую реализацию. **Но Pipeline не меняется ни на строчку.** Это проверка архитектуры: если Dependency Inversion работает, новый адаптер просто встаёт на место.

### Подход: Role как enum, не строка

```rust
enum Role {
    Admin,
    WarehouseManager,
    WarehouseOperator,
    Accountant,
    SalesManager,
    Viewer,
}
```

Строки — это опечатки: `"warehose_manager"` компилируется, но не работает. Enum — exhaustive matching: добавляю новую роль, компилятор показывает все `match`, где она не обработана.

Но есть конфликт: `Role` живёт в `auth`, а `RequestContext` — в `kernel`. Kernel не должен зависеть от auth. Решение: в kernel роли — `Vec<String>`, в auth конвертируем `String → Role` через `Role::from_str_opt()`. Неизвестные роли — игнорируются, не паника. Kernel остаётся чистым.

### Подход: статический RBAC через HashMap

```rust
struct PermissionMap {
    grants: HashMap<Role, HashSet<&'static str>>,
    admin_roles: HashSet<Role>,
}
```

`&'static str` для command names — они известны при компиляции (`"warehouse.receive_goods"`). Нет аллокаций String при каждой проверке.

Wildcard: `"warehouse.*"` → `command_name.starts_with("warehouse.")`. WarehouseManager может всё в своём BC, не перечисляя каждую команду.

Маппинг в коде, не в БД — для MVP достаточно. Когда нужен dynamic RBAC — пишем новый адаптер, загружающий `PermissionMap` из PostgreSQL. Интерфейс тот же.

### Подход: newtype для orphan rule

```rust
pub struct AppErrorResponse(pub AppError);

impl IntoResponse for AppErrorResponse { ... }
```

`AppError` определён в kernel, `IntoResponse` — в axum. Rust запрещает реализовывать чужой trait для чужого типа (orphan rule). Newtype — обёртка, которая «наша», и мы можем реализовать `IntoResponse` для неё.

Маппинг: `Unauthorized → 401`, `Validation → 400`, `NotFound → 404`, `BusinessRule → 422`, `Internal → 500`. Каждый handler просто возвращает `Err(DomainError::NotFound(...))` — middleware превращает в правильный HTTP-ответ.

### Результат

```rust
// Было (тесты без авторизации):
let checker = Arc::new(NoopPermissionChecker);

// Стало (реальная авторизация):
let checker = Arc::new(JwtPermissionChecker::new(default_erp_permissions()));
```

Одна строка. Pipeline не изменился. Integration test в checker.rs подтверждает: unauthorized user → Pipeline возвращает `Err(Unauthorized)`, handler не вызван. Authorized user → handler вызван, результат получен.

---

## Глава 5. Паттерны, которые пронизывают всё

### Arc<dyn Trait> — Dependency Injection в Rust

В C++ DI обычно через указатели: `std::unique_ptr<IService>` или `std::shared_ptr<IService>`. В Rust — `Arc<dyn Trait>`. `Arc` — atomic reference counting (как `shared_ptr`), `dyn Trait` — dynamic dispatch (vtable, как виртуальные методы).

Почему `Arc`, а не `Box`? Потому что Pipeline шарит зависимости между потоками tokio. `Arc` — `Send + Sync`, `Box` — нет (если не Pin).

Почему `dyn Trait`, а не generics? Потому что Pipeline хранит зависимости как поля структуры. Generic'и — compile-time полиморфизм: `Pipeline<Auth, Audit, Hooks>` — каждая комбинация генерирует новый тип. `dyn` — runtime полиморфизм: один тип Pipeline, разные реализации подставляются при создании.

### Send + Sync + 'static — цена многопоточности

Каждый trait в проекте имеет bounds `Send + Sync + 'static`:
- `Send` — можно отправить в другой поток (`tokio::spawn`)
- `Sync` — можно шарить между потоками (`Arc`)
- `'static` — нет borrowed references с ограниченным lifetime

Без этих bounds — `Arc<dyn EventBus>` не компилируется, `tokio::spawn` не принимает future. В C++ это не проблема — потому что в C++ нет compile-time проверки thread safety. В Rust — runtime data races невозможны.

Проект даже содержит compile-time тесты:

```rust
fn _assert_send_sync<T: Send + Sync + 'static>() {}

#[test]
fn traits_are_object_safe_and_send_sync() {
    _assert_send_sync::<Arc<dyn PermissionChecker>>();
    _assert_send_sync::<Arc<dyn EventBus>>();
}
```

Если кто-то сломает bounds — тест не скомпилируется.

### Box<Self> для move semantics в trait objects

```rust
trait UnitOfWork {
    async fn commit(self: Box<Self>) -> Result<(), AppError>;
    async fn rollback(self: Box<Self>) -> Result<(), AppError>;
}
```

`self: Box<Self>` — consuming method на trait object. После `commit()` UoW уничтожается. Нельзя вызвать `commit()` дважды — компилятор запрещает. В C++ это convention («не вызывайте commit после rollback»), в Rust — type system guarantee.

Pipeline вызывает `Box::new(uow).commit()` — ownership передаётся в метод, переменная `uow` больше недоступна.

### as_any_mut — контролируемый downcast

```rust
trait UnitOfWork {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

Handler работает через абстрактный `dyn UnitOfWork`, но ему нужен доступ к PostgreSQL-клиенту для SQL-запросов. `as_any_mut()` + `downcast_mut::<PgUnitOfWork>()` — контролируемый переход от абстракции к конкретике.

Это компромисс. Чистая архитектура говорит «handler не должен знать о PostgreSQL». Реальность говорит «handler должен выполнять SQL». Downcast — явная точка перехода, а не размазанная зависимость.

### tokio::spawn для fire-and-forget

```rust
tokio::spawn(async move {
    if let Err(e) = ext.after_command(&cmd_name, &after_ctx).await {
        error!(command = cmd_name, error = %e, "after_command hook failed");
    }
});
```

After-hook не должен задерживать response. `tokio::spawn` — отдельная задача, Pipeline идёт дальше. Ошибка — только в лог, не в response клиенту. Для email-уведомлений, webhook'ов, аналитики — ровно то, что нужно.

`async move` — closure захватывает `ext`, `cmd_name`, `after_ctx` по значению (move semantics). Без `move` — borrowed references, lifetime проблемы с `tokio::spawn` (requires `'static`).

### Spy и Noop — ручные стабы вместо mock-фреймворков

```rust
pub struct SpyPermissionChecker {
    pub called: Arc<AtomicBool>,
    deny_reason: Option<String>,
}
```

`AtomicBool` — потому что Pipeline async, тест может проверять `called` из другого контекста. `Arc` — shared ownership между spy и тестом. Это проще, предсказуемее и понятнее, чем магия `#[automock]`.

Паттерн: Noop для «мне не важно», Spy для «я проверяю, что это было вызвано».

---

## Итого: ход мыслей

| Шаг | Мысль | Подход |
|-----|-------|--------|
| Порядок слоёв | «Тесты без БД = быстрая итерация» | trait-first, inside-out |
| Kernel | «Общий словарь, не God Object» | Только контракты, без бизнес-типов |
| Newtype ID | «Три Uuid в сигнатуре = баг» | Обёртки с zero-cost |
| Ошибки | «Handler не думает о HTTP» | DomainError → AppError → HTTP status |
| Event Bus | «Шина не знает конкретные типы» | Type erasure через serde_json::Value |
| Handler Registry | «Регистрация 1 раз, dispatch миллионы» | RwLock, не Mutex |
| Pipeline | «Разработчик BC пишет только handler» | Конвейер с trait object зависимостями |
| UoW | «commit дважды = невозможно» | `self: Box<Self>` — consuming method |
| Stubs | «Mock-фреймворк — overkill» | Ручные Noop/Spy на AtomicBool |
| Auth | «Подстановка не меняет Pipeline» | impl Trait for Struct, одна строка замены |
| Role | «Опечатка в строке = баг в проде» | Enum + exhaustive matching |
| Orphan rule | «IntoResponse для чужого AppError» | Newtype обёртка |
| Fire-and-forget | «After-hook не блокирует response» | tokio::spawn + async move |

Общий принцип: **компилятор — первый ревьюер**. Если ошибку можно поймать типами — ловим типами. Если нельзя — пишем тест. Runtime паника — последний вариант.
