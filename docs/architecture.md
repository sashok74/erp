# Архитектура ERP: текущее состояние

> Актуальна на 2026-04-01. Код — source of truth.

---

## 1. Обзор системы

Modular monolith на Rust. Один binary (`gateway`), crate per Bounded Context, PostgreSQL с RLS для tenant isolation.

```mermaid
C4Context
    title ERP System Context

    Person(user, "Пользователь", "Оператор склада, менеджер каталога")
    System(erp, "ERP Gateway", "Rust modular monolith, axum HTTP")
    SystemDb(pg, "PostgreSQL", "Shared DB, schema per BC, RLS")

    Rel(user, erp, "HTTP/JSON", "JWT Bearer")
    Rel(erp, pg, "tokio-postgres", "deadpool connection pool")
```

---

## 2. Граф зависимостей crate'ов

```mermaid
graph TB
    subgraph "Platform SDK"
        kernel["kernel<br/><small>types, traits, errors, security</small>"]
    end

    subgraph "Core Infrastructure"
        event_bus["event_bus<br/><small>EventBus trait, InProcessBus</small>"]
        runtime["runtime<br/><small>CommandPipeline, QueryPipeline,<br/>ports, stubs</small>"]
    end

    subgraph "Data & Auth"
        db["db<br/><small>PgPool, UoW, RLS,<br/>scoped TX, outbox relay</small>"]
        auth["auth<br/><small>JWT, PermissionRegistry,<br/>middleware</small>"]
        audit["audit<br/><small>PgAuditLog,<br/>DomainHistoryWriter</small>"]
        seq_gen["seq_gen<br/><small>gap-free sequences</small>"]
        clorinde["clorinde-gen<br/><small>generated SQL</small>"]
    end

    subgraph "HTTP"
        bc_http["bc_http<br/><small>BcRouter, DTO macros</small>"]
    end

    subgraph "Bounded Contexts"
        warehouse["warehouse<br/><small>InventoryItem, ReceiveGoods,<br/>GetBalance</small>"]
        catalog["catalog<br/><small>Product, CreateProduct,<br/>GetProduct</small>"]
    end

    subgraph "Composition Root"
        gateway["gateway<br/><small>AppBuilder, main.rs</small>"]
    end

    event_bus --> kernel
    runtime --> kernel
    runtime --> event_bus
    db --> kernel
    db --> runtime
    db --> event_bus
    db --> clorinde
    auth --> kernel
    auth --> runtime
    audit --> kernel
    audit --> runtime
    audit --> db
    audit --> clorinde
    seq_gen --> kernel
    seq_gen --> clorinde
    bc_http --> kernel
    bc_http --> runtime

    warehouse --> kernel
    warehouse --> runtime
    warehouse --> event_bus
    warehouse --> bc_http
    warehouse --> db
    warehouse --> clorinde
    warehouse --> audit
    warehouse --> seq_gen

    catalog --> kernel
    catalog --> runtime
    catalog --> event_bus
    catalog --> bc_http
    catalog --> db
    catalog --> clorinde
    catalog --> audit

    gateway --> kernel
    gateway --> runtime
    gateway --> event_bus
    gateway --> db
    gateway --> auth
    gateway --> audit
    gateway --> warehouse
    gateway --> catalog

    style kernel fill:#e1f5fe
    style warehouse fill:#e8f5e9
    style catalog fill:#fff3e0
    style gateway fill:#fce4ec
```

---

## 3. Луковая архитектура внутри BC

```mermaid
graph TB
    subgraph "Bounded Context (warehouse / catalog)"
        subgraph "Domain Layer"
            agg["Aggregates<br/><small>InventoryItem, Product</small>"]
            vo["Value Objects<br/><small>Sku, Quantity, ProductName</small>"]
            evt["Domain Events<br/><small>GoodsReceived, ProductCreated</small>"]
            err["Domain Errors<br/><small>InsufficientStock, InvalidSku</small>"]
        end

        subgraph "Application Layer"
            cmd["Command Handlers<br/><small>ReceiveGoodsHandler</small>"]
            qry["Query Handlers<br/><small>GetBalanceHandler</small>"]
            ports["Ports (repo structs)<br/><small>InventoryRepo, ProductRepo</small>"]
        end

        subgraph "Infrastructure Layer"
            repos["Repo Implementations<br/><small>impl InventoryRepo (clorinde SQL)</small>"]
            http["HTTP Routes<br/><small>BcRouter, DTOs</small>"]
            eh["Event Handlers<br/><small>ProductCreatedHandler</small>"]
        end
    end

    cmd --> agg
    cmd --> vo
    cmd --> ports
    qry --> ports

    repos -.->|"split impl"| ports
    http --> cmd
    http --> qry
    eh --> evt

    style agg fill:#e8f5e9
    style vo fill:#e8f5e9
    style evt fill:#e8f5e9
    style err fill:#e8f5e9
    style cmd fill:#e1f5fe
    style qry fill:#e1f5fe
    style ports fill:#e1f5fe
    style repos fill:#fff3e0
    style http fill:#fff3e0
    style eh fill:#fff3e0
```

**Split impl pattern:** struct `InventoryRepo` определён в `application/ports.rs`, методы реализованы в `infrastructure/repos.rs`. Handler импортирует из своего слоя.

---

## 4. Canonical Write Path (Command Pipeline)

```mermaid
sequenceDiagram
    participant H as HTTP Client
    participant MW as Auth Middleware
    participant P as CommandPipeline
    participant PC as PermissionChecker
    participant UoW as PgUnitOfWork
    participant Handler as CommandHandler
    participant Repo as Repository
    participant DB as PostgreSQL
    participant A as AuditLog
    participant R as OutboxRelay
    participant Bus as EventBus

    H->>MW: POST /api/warehouse/receive
    MW->>MW: JWT verify → RequestContext
    MW->>P: execute(handler, cmd, ctx)

    P->>PC: check_permission(ctx, "warehouse.receive_goods")
    PC-->>P: Ok

    P->>UoW: begin(ctx)
    UoW->>DB: BEGIN
    UoW->>DB: SET LOCAL app.tenant_id = '...'
    UoW-->>P: &mut UoW

    P->>Handler: handle(cmd, ctx, &mut uow)
    Handler->>Repo: find_by_sku(client, tenant_id, sku)
    Repo->>DB: SELECT (clorinde, RLS applied)
    DB-->>Repo: row
    Handler->>Handler: domain logic (aggregate.receive())
    Handler->>Repo: save_movement, upsert_balance
    Repo->>DB: INSERT/UPSERT
    Handler->>UoW: record_change (deferred)
    Handler->>UoW: emit_events (deferred)
    Handler-->>P: Ok(result)

    P->>UoW: commit()
    UoW->>DB: INSERT domain_history
    UoW->>DB: INSERT outbox
    UoW->>DB: COMMIT

    P->>A: log(ctx, cmd_name, result)
    A->>DB: INSERT audit_log (separate TX)

    P-->>MW: Ok(result)
    MW-->>H: 200 OK {result}

    Note over R,Bus: Background (async)
    R->>DB: SELECT FROM outbox WHERE NOT published
    R->>Bus: publish_and_wait(event)
    Bus->>Bus: InboxAwareHandler (dedup check)
    R->>DB: UPDATE outbox SET published = true
```

---

## 5. Read Path (Query Pipeline)

```mermaid
sequenceDiagram
    participant H as HTTP Client
    participant MW as Auth Middleware
    participant P as QueryPipeline
    participant PC as PermissionChecker
    participant EH as ExtensionHooks
    participant Handler as QueryHandler
    participant DB as PostgreSQL
    participant A as AuditLog

    H->>MW: GET /api/warehouse/balance?sku=BOLT-42
    MW->>MW: JWT verify → RequestContext
    MW->>P: execute(handler, query, ctx)

    P->>PC: check_permission(ctx, "warehouse.get_balance")
    PC-->>P: Ok

    P->>EH: before_query(query_name, ctx)
    EH-->>P: Ok

    P->>Handler: handle(query, ctx)

    Note over Handler,DB: with_tenant_read(pool, tenant_id, closure)
    Handler->>DB: BEGIN READ ONLY
    Handler->>DB: SET LOCAL app.tenant_id = '...'
    Handler->>DB: SELECT (clorinde, RLS applied)
    DB-->>Handler: rows
    Handler->>DB: COMMIT
    Handler-->>P: Ok(result)

    P->>EH: after_query (fire-and-forget, tokio::spawn)
    P->>A: log(ctx, query_name, result)

    P-->>MW: Ok(result)
    MW-->>H: 200 OK {result}
```

---

## 6. Cross-BC Event Flow

```mermaid
sequenceDiagram
    participant Cat as Catalog Handler
    participant DB as PostgreSQL
    participant Relay as OutboxRelay
    participant Bus as EventBus
    participant Inbox as InboxAwareHandler
    participant WH as ProductCreatedHandler
    participant Proj as warehouse.product_projections

    Cat->>DB: INSERT catalog.products
    Cat->>DB: INSERT common.outbox (ProductCreated)
    Cat->>DB: COMMIT (atomic)

    Note over Relay: Background poll
    Relay->>DB: SELECT FROM outbox (FOR UPDATE SKIP LOCKED)
    Relay->>Bus: publish_and_wait(ProductCreated)

    Bus->>Inbox: handle_envelope(event)
    Inbox->>DB: CHECK common.inbox (event_id, handler_name)
    Note over Inbox: Not processed yet
    Inbox->>WH: handle(ProductCreatedEvent)

    Note over WH,Proj: with_tenant_write(pool, tenant_id, closure)
    WH->>DB: BEGIN + SET LOCAL tenant_id
    WH->>Proj: UPSERT product_projections
    WH->>DB: COMMIT

    Inbox->>DB: INSERT common.inbox (mark processed)
    Relay->>DB: UPDATE outbox SET published = true
```

---

## 7. Tenant Isolation (RLS)

```mermaid
graph LR
    subgraph "Write Path"
        W1["PgUnitOfWork::begin()"]
        W2["BEGIN"]
        W3["SET LOCAL app.tenant_id"]
        W4["Handler SQL"]
        W5["COMMIT"]
        W1 --> W2 --> W3 --> W4 --> W5
    end

    subgraph "Read Path"
        R1["with_tenant_read()"]
        R2["BEGIN READ ONLY"]
        R3["SET LOCAL app.tenant_id"]
        R4["Handler SQL"]
        R5["COMMIT"]
        R1 --> R2 --> R3 --> R4 --> R5
    end

    subgraph "Event Handler Path"
        E1["with_tenant_write()"]
        E2["BEGIN"]
        E3["SET LOCAL app.tenant_id"]
        E4["Projection SQL"]
        E5["COMMIT"]
        E1 --> E2 --> E3 --> E4 --> E5
    end

    subgraph "PostgreSQL"
        RLS["RLS Policy:<br/>tenant_id = current_setting('app.tenant_id')"]
        FORCE["FORCE RLS:<br/>business tables + audit/sequences/history"]
        NOFORCE["Без FORCE:<br/>outbox, dead_letters<br/>(relay читает cross-tenant)"]
    end

    W4 -.->|"фильтрует"| RLS
    R4 -.->|"фильтрует"| RLS
    E4 -.->|"фильтрует"| RLS
    RLS --- FORCE
    RLS --- NOFORCE

    style RLS fill:#ffcdd2
    style FORCE fill:#ffcdd2
    style NOFORCE fill:#fff9c4
```

---

## 8. RBAC: BC-Owned Permissions

```mermaid
graph TB
    subgraph "Startup"
        WP["WarehousePermissions<br/>.permission_manifest()"]
        CP["CatalogPermissions<br/>.permission_manifest()"]
        V["Validate:<br/>namespace, duplicates,<br/>platform role collisions"]
        REG["PermissionRegistry"]

        WP --> V
        CP --> V
        V --> REG
    end

    subgraph "Runtime (per request)"
        JWT["JWT Token<br/>{roles: ['warehouse_operator']}"]
        CHK["JwtPermissionChecker"]
        ACT["action: 'warehouse.receive_goods'"]
        RES{{"Allowed?"}}

        JWT --> CHK
        ACT --> CHK
        CHK --> REG
        REG --> RES
    end

    subgraph "Grants"
        G1["admin → *(superadmin)"]
        G2["warehouse_manager → warehouse.*"]
        G3["warehouse_operator → warehouse.receive_goods,<br/>warehouse.get_balance"]
        G4["catalog_manager → catalog.*"]
        G5["viewer → catalog.get_product,<br/>warehouse.get_balance"]
    end

    RES -->|"Yes"| OK["Pipeline continues"]
    RES -->|"No"| ERR["401 Unauthorized"]

    style REG fill:#e1f5fe
    style RES fill:#fff9c4
```

---

## 9. Database Schema

Схема получена из реальной БД через MCP (`erp-dev-pg`).

```mermaid
erDiagram
    COMMON_TENANTS {
        uuid id PK
        text name
        text slug UK
        bool is_active
        timestamptz created_at
        timestamptz updated_at
    }

    COMMON_OUTBOX {
        bigint id PK
        uuid tenant_id
        uuid event_id UK
        text event_type
        text source
        jsonb payload
        uuid correlation_id
        uuid causation_id
        uuid user_id
        timestamptz created_at
        bool published
        timestamptz published_at
        int retry_count
    }

    COMMON_INBOX {
        uuid event_id PK
        text handler_name PK
        text event_type
        text source
        timestamptz processed_at
    }

    COMMON_AUDIT_LOG {
        bigint id PK
        uuid tenant_id
        uuid user_id
        uuid correlation_id
        uuid causation_id
        text command_name
        jsonb result
        timestamptz created_at
    }

    COMMON_DOMAIN_HISTORY {
        bigint id PK
        uuid tenant_id
        text entity_type
        uuid entity_id
        text event_type
        jsonb old_state
        jsonb new_state
        uuid correlation_id
        uuid causation_id
        uuid user_id
        timestamptz created_at
    }

    COMMON_DEAD_LETTERS {
        bigint id PK
        uuid event_id UK
        text event_type
        text source
        uuid tenant_id
        jsonb payload
        uuid correlation_id
        uuid causation_id
        uuid user_id
        timestamptz original_created_at
        timestamptz failed_at
        int retry_count
        text last_error
    }

    COMMON_SEQUENCES {
        uuid tenant_id PK
        text seq_name PK
        text prefix
        bigint next_value
    }

    WAREHOUSE_INVENTORY_ITEMS {
        uuid tenant_id PK
        uuid id PK
        text sku "UK(tenant_id, sku)"
        timestamptz created_at
    }

    WAREHOUSE_STOCK_MOVEMENTS {
        uuid tenant_id PK
        uuid id PK
        uuid item_id
        text event_type
        numeric_18_4 quantity
        numeric_18_4 balance_after
        text doc_number
        uuid correlation_id
        uuid user_id
        timestamptz created_at
    }

    WAREHOUSE_INVENTORY_BALANCES {
        uuid tenant_id PK
        uuid item_id PK
        text sku
        numeric_18_4 balance
        uuid last_movement_id
        timestamptz updated_at
    }

    WAREHOUSE_PRODUCT_PROJECTIONS {
        uuid tenant_id PK
        uuid product_id PK
        text sku "IDX(tenant_id, sku)"
        text name
        text category
        timestamptz updated_at
    }

    CATALOG_PRODUCTS {
        uuid tenant_id PK
        uuid id PK
        text sku "UK(tenant_id, sku)"
        text name
        text category
        text unit
        timestamptz created_at
        timestamptz updated_at
    }

    WAREHOUSE_INVENTORY_ITEMS ||--o{ WAREHOUSE_STOCK_MOVEMENTS : "item_id"
    WAREHOUSE_INVENTORY_ITEMS ||--o| WAREHOUSE_INVENTORY_BALANCES : "item_id"
    CATALOG_PRODUCTS ||--o| WAREHOUSE_PRODUCT_PROJECTIONS : "event projection"
    COMMON_OUTBOX ||--o| COMMON_DEAD_LETTERS : "event_id (при max retries)"
```

Все UNIQUE-ограничения на `sku` — tenant-scoped: `UNIQUE(tenant_id, sku)`, не глобальные.

---

## 10. Gateway Assembly

```mermaid
graph TB
    subgraph "main.rs startup"
        CFG["Config from env"]
        POOL["PgPool (deadpool, max 20)"]
        BUS["InProcessBus"]
        INBOX_BUS["InboxBusDecorator<br/>(wraps BUS + inbox dedup)"]
        JWT_SVC["JwtService (HS256)"]

        RBAC["PermissionRegistry<br/>(from BC manifests)"]
        CHK["JwtPermissionChecker"]
        AUDIT["PgAuditLog"]
        UOW_F["PgUnitOfWorkFactory"]

        CMD_P["CommandPipeline"]
        QRY_P["QueryPipeline"]

        AB["AppBuilder"]

        CFG --> POOL
        CFG --> JWT_SVC
        POOL --> UOW_F
        POOL --> AUDIT
        BUS --> INBOX_BUS
        RBAC --> CHK

        UOW_F --> CMD_P
        BUS --> CMD_P
        CHK --> CMD_P
        CHK --> QRY_P
        AUDIT --> CMD_P
        AUDIT --> QRY_P
    end

    subgraph "BC Registration"
        WH_M["WarehouseModule"]
        CAT_M["CatalogModule"]

        WH_M -->|"migrations"| AB
        WH_M -->|"event handlers (subscribe via InboxBus)"| AB
        WH_M -->|"routes /api/warehouse"| AB

        CAT_M -->|"migrations"| AB
        CAT_M -->|"event handlers (subscribe via InboxBus)"| AB
        CAT_M -->|"routes /api/catalog"| AB
    end

    subgraph "Background"
        RELAY["OutboxRelay<br/>(tokio::spawn)"]
        RELAY -->|"poll outbox"| POOL
        RELAY -->|"publish events"| INBOX_BUS
    end

    subgraph "HTTP Router"
        HEALTH["/health, /ready"]
        DEV["/dev/token, /dev/events"]
        API["/api/warehouse/**, /api/catalog/**"]
        MW["auth_middleware (JWT)"]

        MW --> API
    end

    AB --> API
    CMD_P --> AB
    QRY_P --> AB
    POOL --> AB

    style CMD_P fill:#e1f5fe
    style QRY_P fill:#e1f5fe
    style RELAY fill:#fff3e0
    style MW fill:#ffcdd2
```

---

## 11. Outbox Relay + Inbox Dedup

```mermaid
stateDiagram-v2
    [*] --> Unpublished: handler commits (outbox INSERT)

    state OutboxRelay {
        Unpublished --> Publishing: relay poll (FOR UPDATE SKIP LOCKED)
        Publishing --> Published: bus.publish_and_wait() OK
        Publishing --> RetryPending: publish error (retry < 3)
        RetryPending --> Publishing: next poll cycle
        Publishing --> DeadLetter: retry >= 3
    }

    state InboxDedup {
        Published --> CheckInbox: handler receives event
        CheckInbox --> Skipped: already processed
        CheckInbox --> Processing: not processed
        Processing --> Processed: handler OK + mark_processed
        Processing --> RetryLater: handler error (inbox NOT marked)
    }

    Published --> [*]
    Skipped --> [*]
    Processed --> [*]
    DeadLetter --> [*]
```

---

## 12. Структура файлов BC (шаблон)

```
crates/{bc_name}/
├── src/
│   ├── lib.rs                          # pub mod application, domain, infrastructure
│   ├── module.rs                       # impl BoundedContextModule
│   ├── registrar.rs                    # impl PermissionRegistrar
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── aggregates.rs              # Rich aggregates + AggregateRoot impl
│   │   ├── events.rs                  # DomainEvent impl (erp.{bc}.{event}.v{N})
│   │   ├── value_objects.rs           # Sku, Quantity, etc. (validation)
│   │   └── errors.rs                  # BC-specific DomainError
│   ├── application/
│   │   ├── mod.rs
│   │   ├── ports.rs                   # Repo struct + DTO (split impl pattern)
│   │   ├── commands/
│   │   │   └── {command}.rs           # Command + CommandHandler
│   │   └── queries/
│   │       └── {query}.rs             # Query + QueryHandler
│   └── infrastructure/
│       ├── mod.rs
│       ├── repos.rs                   # impl ports::Repo (clorinde SQL)
│       ├── event_handlers.rs          # Integration event subscribers
│       ├── http.rs                    # BcRouter + DTO macros
│       └── routes.rs                  # axum Router builder
├── tests/
│   └── integration.rs                 # E2E tests with real PostgreSQL
├── Cargo.toml
└── BC_CONTEXT.md                      # BC passport (aggregates, events, tables)
```

---

## 13. Ключевые архитектурные решения

| Решение | Обоснование |
|---------|-------------|
| Modular monolith | Один binary, split to microservices позже через EventBus trait |
| PostgreSQL shared DB + schema per BC | Одна БД, изоляция через RLS + schema namespace |
| Clorinde (SQL-first) | SQL в файлах, codegen без БД при компиляции |
| Transactional outbox | Атомарность: бизнес-данные + events в одной TX |
| Inbox dedup | At-least-once delivery + per-handler dedup; handler'ы должны быть идемпотентны (UPSERT) |
| FORCE RLS | Owner подчиняется RLS на business + audit таблицах; outbox/dead_letters без FORCE (relay читает cross-tenant) |
| Closure-based TX (`with_tenant_read/write`) | Невозможно забыть BEGIN/COMMIT |
| Split impl для repos | Onion architecture без trait overhead |
| BC-owned RBAC | Каждый BC декларирует свои роли/permissions |
| UUID v7 | Time-ordered для лучшей производительности B-tree индексов |
| BigDecimal в BC | Точная арифметика для денег/количеств |
