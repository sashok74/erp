# Rust-ликбез для C++ программиста — Step 2: Глубже в семантику

> Примеры взяты из тех же файлов (`crates/kernel/`), но разбирают **концепции**,
> которые step1 показал поверхностно или пропустил.
> Фокус — на том, что **ломает привычки C++**.

---

## 1. Ownership и Move: «по умолчанию — перемещение»

```rust
let event = Incremented { value: 5, aggregate: uuid };
self.events.push(event);
// event больше НЕ существует — он перемещён в вектор
```

| Что происходит | Rust | C++ |
|----------------|------|-----|
| `let event = Incremented { ... }` | `event` **владеет** значением | `auto event = Incremented{...};` — значение на стеке |
| `self.events.push(event)` | `event` **перемещён** в вектор. Переменная «мертва» | `vec.push_back(std::move(event));` — но в C++ move нужен **явно** |
| Доступ к `event` после push | **Ошибка компиляции**: `use of moved value` | Компилируется! Но UB / unspecified state |

**Ключевое отличие от C++:**

В C++ `auto x = y;` **копирует** (если есть copy constructor). Move делается только через `std::move(y)`.

В Rust `let x = y;` **перемещает**. Копия — только если тип реализует `Copy` (числа, `bool`, ссылки). Для всего остального — нужен явный `.clone()`.

```rust
let a = String::from("hello");
let b = a;          // a перемещён в b
// println!("{a}");  // ОШИБКА: a больше не существует

let c = b.clone();  // явная глубокая копия
println!("{b}");     // OK — b всё ещё жив
println!("{c}");     // OK — c — независимая копия
```

**Аналогия для C++:** представь, что в C++ **все типы по умолчанию move-only** (удалён copy constructor), а `std::move()` вызывается автоматически при присваивании. Хочешь копию — пиши `.clone()` (аналог явного copy constructor).

---

## 2. `let mut` — мутабельность как осознанный выбор

```rust
let mut counter = Counter::new();   // мутабельная переменная
counter.increment(5);               // OK — counter мутабелен

let id = TenantId::new();           // иммутабельная
// id = TenantId::new();            // ОШИБКА: cannot assign twice to immutable variable
```

| Rust | C++ | Что значит |
|------|-----|------------|
| `let x = ...;` | `const auto x = ...;` | Иммутабельно. Нельзя ни менять, ни вызывать `&mut self` методы |
| `let mut x = ...;` | `auto x = ...;` | Мутабельно |

**Инверсия по умолчанию:** в C++ по умолчанию всё мутабельно, `const` нужно добавлять. В Rust наоборот — по умолчанию всё иммутабельно, `mut` нужно добавлять.

Почему так: неизменяемость по умолчанию делает код предсказуемым. Компилятор знает, что если переменная не `mut`, она не изменится — и может оптимизировать / гарантировать thread safety.

---

## 3. Borrowing: `&` и `&mut` — правила заимствования

```rust
fn apply(&mut self, event: &Self::Event) {
    self.total += event.value;
}
```

| Rust | C++ | Правило |
|------|-----|---------|
| `&self` | `const T& self` / `const T* this` | Неизменяемая ссылка. Можно иметь **сколько угодно** одновременно |
| `&mut self` | `T& self` / `T* this` | Изменяемая ссылка. Можно иметь **ровно одну**, и **никаких** `&` в это время |
| `event: &Self::Event` | `const Event& event` | Заимствуем событие на чтение |

**Правила заимствования (Borrow Checker):**

1. **Или** одна `&mut` ссылка, **или** любое количество `&` ссылок — никогда оба одновременно
2. Ссылка не может жить дольше, чем данные, на которые указывает

```rust
let mut v = vec![1, 2, 3];
let first = &v[0];     // иммутабельное заимствование
v.push(4);             // ОШИБКА! v.push() берёт &mut v, а first ещё жива
println!("{first}");   // ← first используется здесь
```

**Аналогия в C++:** это как если бы `const_cast`, алиасинг через `T*` и `const T*`, invalidation итераторов — **всё проверялось на этапе компиляции**. В C++ `push_back` может инвалидировать указатель на `v[0]`, и это UB. В Rust — ошибка компиляции.

---

## 4. `String` vs `&str` — два типа строк

```rust
pub source: String,                          // владеющая строка (в структуре)
pub fn new(source: &str, ...) -> Self { }    // строковый срез (аргумент)
source: source.to_string(),                  // конвертация &str → String
```

| Rust | C++ | Где живёт | Владеет данными? |
|------|-----|-----------|------------------|
| `String` | `std::string` | Heap (куча) | **Да** — владеет буфером |
| `&str` | `std::string_view` | Где угодно (стек, heap, .rodata) | **Нет** — ссылка на чужие данные |
| `"literal"` | `"literal"` | Статическая память (.rodata) | Тип: `&'static str` / `const char*` |

**Правило:**
- Поля структур → `String` (владеют данными, struct может жить сколько угодно)
- Аргументы функций → `&str` (не важно откуда строка — примут и `&String`, и литерал)
- Возвращаемые значения → `String` (если создаём новую) или `&str` (если ссылаемся на существующую)

```rust
fn greet(name: &str) -> String {         // принимаем срез, возвращаем владеющую
    format!("Hello, {name}")             // format! создаёт новую String
}

let s = String::from("Alice");
greet(&s);       // &String автоматически приводится к &str (Deref coercion)
greet("Bob");    // &str литерал — и так подходит
```

**Аналогия в C++:** `std::string` и `std::string_view` — почти 1:1. Но в C++ `string_view` может стать dangling, и компилятор **не предупредит**. В Rust lifetime checker **гарантирует**, что `&str` не переживёт данные.

---

## 5. `.to_string()` — зачем и когда

```rust
specversion: "1.0".to_string(),
source: source.to_string(),
event_type: event_type.to_string(),
```

| Выражение | Что происходит | Аналог в C++ |
|-----------|----------------|--------------|
| `"1.0".to_string()` | Литерал `&'static str` → `String` (копирует в heap) | `std::string("1.0")` — конструирует из `const char*` |
| `source.to_string()` | `&str` → `String` (копирует) | `std::string(sv)` из `string_view` |
| `String::from("1.0")` | То же самое, альтернативный синтаксис | Идентично |
| `format!("v{}", 1)` | Форматированная строка → `String` | `std::format("v{}", 1)` (C++20) |

**Почему это нужно:** если поле структуры типа `String`, а у тебя `&str` — нужно **явно** создать владеющую копию. Rust не делает неявных аллокаций. В C++ `std::string name = "hello";` вызывает аллокацию **неявно** через implicit constructor.

---

## 6. `Vec<T>` — динамический массив

```rust
events: Vec<Incremented>,                   // объявление поля
events: Vec::new(),                         // создание пустого вектора
self.events.push(event);                    // добавление (move элемента!)
let events = counter.take_events();         // перемещение всего вектора
assert_eq!(events.len(), 2);               // длина
assert_eq!(events[0].value, 5);            // индексация (паника при out-of-bounds!)
assert!(events_again.is_empty());           // проверка на пустоту
```

| Rust | C++ | Комментарий |
|------|-----|-------------|
| `Vec<T>` | `std::vector<T>` | Практически идентично |
| `Vec::new()` | `std::vector<T>{}` | Пустой вектор, без аллокации (аллокация при первом push) |
| `.push(x)` | `.push_back(std::move(x))` | В Rust **перемещает** `x` в вектор. Всегда |
| `v[i]` | `v[i]` | Доступ по индексу. **Паника** при out-of-bounds (не UB как в C++) |
| `.len()` | `.size()` | Количество элементов |
| `.is_empty()` | `.empty()` | `true` если пусто |
| `v.get(i)` | — | Безопасный доступ: возвращает `Option<&T>` вместо паники |

**Ключевое отличие:** `v[i]` в C++ — UB при выходе за границы. В Rust — гарантированная паника (bounds check). Если не хочешь паники — `v.get(i)` вернёт `Option::None`.

---

## 7. `Ok(...)`, `Err(...)`, `Some(...)`, `None` — конструкторы

```rust
fn fallible() -> Result<(), AppError> {
    Err(DomainError::NegativeBalance)?     // Err(...) — создаём ошибку
}

let err = fallible().unwrap_err();         // извлекаем Err
```

```rust
subject: Some("agg-123".to_string()),      // значение есть
subject: None,                              // значения нет
```

| Rust | C++ | Что это |
|------|-----|---------|
| `Ok(value)` | `std::expected<T,E>{value}` | Создаёт успешный `Result` |
| `Err(error)` | `std::unexpected(error)` | Создаёт ошибочный `Result` |
| `Some(value)` | `std::optional<T>{value}` | Создаёт непустой `Option` |
| `None` | `std::nullopt` | Пустой `Option` |
| `.unwrap_err()` | — | Извлекает `Err`. **Паникует** если `Ok` |
| `.unwrap()` | `.value()` | Извлекает `Ok`/`Some`. **Паникует** если `Err`/`None` |

**Отличие от C++:** `Result` и `Option` — это **enum'ы** (алгебраические типы). `Ok`, `Err`, `Some`, `None` — их варианты. Нет отдельных классов — это всё один тип.

```rust
// Result определён в std как:
enum Result<T, E> {
    Ok(T),
    Err(E),
}

// Option определён как:
enum Option<T> {
    Some(T),
    None,
}
```

Это значит, что `match` по `Result` и `Option` — обычный pattern matching по enum, а не специальная магия.

---

## 8. Целочисленные типы — без неявных конверсий

```rust
pub total: i32,                            // знаковое 32-бит
pub expected: i64,                         // знаковое 64-бит
pub quantity: i32,                         // знаковое 32-бит
assert_eq!(events.len(), 2);              // len() возвращает usize
```

| Rust | C++ | Размер |
|------|-----|--------|
| `i8` / `u8` | `int8_t` / `uint8_t` | 8 бит |
| `i16` / `u16` | `int16_t` / `uint16_t` | 16 бит |
| `i32` / `u32` | `int32_t` / `uint32_t` | 32 бит |
| `i64` / `u64` | `int64_t` / `uint64_t` | 64 бит |
| `i128` / `u128` | `__int128` | 128 бит |
| `isize` / `usize` | `intptr_t` / `size_t` | Размер указателя |
| `f32` / `f64` | `float` / `double` | IEEE 754 |

**Ключевое отличие — НИКАКИХ неявных конверсий:**

```rust
let x: i32 = 42;
let y: i64 = x;        // ОШИБКА! В C++ сработало бы (implicit widening)
let y: i64 = i64::from(x);  // OK — явная конвертация
let y: i64 = x.into();      // OK — то же через trait Into

let big: i64 = 1000;
let small: i32 = big as i32; // OK — но `as` может обрезать! Как C-style cast
```

В C++ неявные конверсии `int` → `long`, `int` → `double`, `unsigned` → `signed` — источник огромного количества багов. Rust требует **явность**: `From`/`Into` для безопасных конверсий, `as` для потенциально теряющих.

---

## 9. Операторы — через trait'ы, а не магия

```rust
self.total += event.value;                 // operator+=
assert_eq!(id, deserialized);             // operator==
let elapsed = Utc::now() - ctx.timestamp; // operator-
```

| Оператор | Rust trait | C++ | Как получить |
|----------|-----------|-----|-------------|
| `==`, `!=` | `PartialEq` | `operator==` | `#[derive(PartialEq)]` |
| `<`, `>`, `<=`, `>=` | `PartialOrd` | `operator<=>` (C++20) | `#[derive(PartialOrd)]` |
| `+` | `Add` | `operator+` | `impl Add for T` |
| `-` | `Sub` | `operator-` | `impl Sub for T` |
| `+=` | `AddAssign` | `operator+=` | `impl AddAssign for T` |
| `[]` | `Index` / `IndexMut` | `operator[]` | `impl Index for T` |
| `*x` (deref) | `Deref` / `DerefMut` | `operator*` | `impl Deref for T` |

**Отличие от C++:** операторы — обычные trait'ы. `derive` генерирует их автоматически. Нельзя перегрузить оператор «в классе» — только через `impl Trait for Type`.

Для примитивов (`i32 += i32`) реализации встроены в компилятор.

---

## 10. Method chaining — цепочки вызовов

```rust
assert_eq!(id.as_uuid().get_version_num(), 7);
//         │            │                  │
//         │            └─ &Uuid → usize   │
//         └─ &TenantId → &Uuid            │
//                                         └─ значение для сравнения
```

| Шаг | Возвращает | C++ аналог |
|-----|-----------|------------|
| `id` | `TenantId` (по значению или по ссылке) | `id` |
| `.as_uuid()` | `&Uuid` — ссылка на внутренний UUID | `id.as_uuid()` → `const Uuid&` |
| `.get_version_num()` | `usize` — число (Copy, значение) | `.get_version_num()` → `size_t` |

Метод можно вызвать на ссылке — Rust автоматически разыменовывает (`auto-deref`). Если `as_uuid()` вернул `&Uuid`, можно сразу вызвать метод `Uuid` — компилятор сам вставит `(*ref).method()`.

**Аналогия в C++:** `id->as_uuid().get_version_num()` — только в C++ нужен `->` для указателей, а `.` для значений. В Rust **всегда `.`** — компилятор разберётся.

---

## 11. Вложенная функция (nested function)

```rust
#[test]
fn domain_error_converts_to_app_error_via_from() {
    fn fallible() -> Result<(), AppError> {   // функция внутри функции!
        Err(DomainError::NegativeBalance)?
    }

    let err = fallible().unwrap_err();
    assert!(matches!(err, AppError::Domain(DomainError::NegativeBalance)));
}
```

| Rust | C++ |
|------|-----|
| `fn` внутри `fn` — обычная вложенная функция | Лямбда `auto fallible = [&]() -> expected<...> { ... };` |

В Rust можно определить обычную `fn` внутри другой `fn`. Она **не захватывает** окружение (в отличие от замыканий/closures). Это просто локальная функция, невидимая снаружи.

В C++ вложенные функции запрещены (кроме лямбд и локальных классов с `operator()`).

---

## 12. `format!` и строковое форматирование

```rust
write!(f, "{}", self.0)                    // вставка значения с Display
println!("ERP Gateway — placeholder");     // печать с \n
format!("Hello, {name}")                   // создание String (не печать!)
```

| Макрос | Что делает | C++ аналог |
|--------|-----------|------------|
| `format!(...)` | Создаёт `String` | `std::format(...)` (C++20) |
| `println!(...)` | Печать в stdout + `\n` | `std::println(...)` (C++23) или `std::cout <<` |
| `eprintln!(...)` | Печать в stderr + `\n` | `std::cerr <<` |
| `write!(f, ...)` | Запись в буфер `f` | `std::format_to(buf, ...)` |

Плейсхолдеры:

| Rust | C++ | Что делает |
|------|-----|------------|
| `{}` | `{}` (C++20 format) | `Display` trait — человекочитаемый вывод |
| `{:?}` | — | `Debug` trait — отладочный вывод (как `repr` в Python) |
| `{name}` | `{name}` (нет в C++20) | Подстановка переменной по имени (Rust 1.58+) |
| `{0}` | `{0}` | Подстановка по позиции |

**Отличие от C++:** `format!`, `println!` — это **макросы** (с `!`). Они проверяют формат-строку **на этапе компиляции**. Несоответствие типов — ошибка компиляции, не runtime UB как в `printf`.

---

## 13. Cargo.toml и Workspace — система сборки

### Workspace (корневой `Cargo.toml`)

```toml
[workspace]
resolver = "2"
members = [
    "crates/kernel",
    "crates/db",
    "crates/gateway",
]
```

| Секция | Значение | Аналог в C++ |
|--------|----------|--------------|
| `[workspace]` | Объединяет несколько crate'ов в одном проекте | CMake top-level `CMakeLists.txt` с `add_subdirectory()` |
| `resolver = "2"` | Алгоритм разрешения зависимостей (v2 — учитывает features per platform) | Нет аналога |
| `members = [...]` | Список crate'ов в workspace | `add_subdirectory(crates/kernel)` |

### Общие зависимости

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v7", "serde"] }

# Internal crates
kernel = { path = "crates/kernel" }
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `[workspace.dependencies]` | Версии зависимостей задаются **один раз** для всего workspace | CMake `FetchContent` или Conan `conanfile.txt` на уровне проекта |
| `version = "1"` | SemVer: `>=1.0.0, <2.0.0` | Conan version range |
| `features = ["derive"]` | Условная компиляция: включить фичу `derive` для serde | CMake `option()` / `target_compile_definitions()` |
| `path = "crates/kernel"` | Локальный crate (не из registry) | `add_subdirectory(crates/kernel)` |

### Crate Cargo.toml

```toml
[package]
name = "kernel"
version.workspace = true
edition.workspace = true

[dependencies]
uuid = { workspace = true }
serde = { workspace = true }
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `[package]` | Метаданные crate | `project()` в CMakeLists.txt |
| `version.workspace = true` | «Бери версию из workspace» | Переменная `${PROJECT_VERSION}` в CMake |
| `uuid = { workspace = true }` | «Бери версию uuid из workspace dependencies» | Ссылка на imported target из parent |
| `publish = false` | Не публиковать в crates.io | Нет аналога |

**Ключевое отличие от C++:** Cargo — **единственная** система сборки + менеджер пакетов + test runner + doc generator. В C++ для этого нужны CMake + Conan/vcpkg + CTest + Doxygen — 4 отдельных инструмента.

---

## 14. `edition = "2024"` — что такое Edition

```toml
edition = "2024"
rust-version = "1.85"
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `edition` | «Диалект» языка. Может менять синтаксис (новые keywords, правила) | `-std=c++20`, `-std=c++23` |
| `rust-version` | Минимальная версия компилятора | `cmake_minimum_required(VERSION ...)` |

**Отличие от C++:** editions в Rust **обратно совместимы на уровне линковки**. Crate на edition 2021 и crate на edition 2024 могут линковаться вместе. В C++ смешивание `-std=c++17` и `-std=c++20` в одном проекте — рецепт проблем.

---

## 15. `features` — условная компиляция зависимостей

```toml
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v7", "serde"] }
sqlx = { version = "0.8", features = [
    "runtime-tokio-rustls", "postgres", "uuid", "chrono",
    "json", "migrate", "bigdecimal"
] }
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `features = ["derive"]` | Включить модуль derive-макросов в serde | `-DSERDE_DERIVE=ON` в CMake |
| `features = ["v7"]` | Включить поддержку UUID v7 | `#define UUID_V7_SUPPORT` |
| Без фичи | Код не компилируется, не линкуется | `#ifdef` отсекает код |

Features — это **additive compilation**. Каждая feature включает дополнительный код. Нельзя отключить фичу, которую включил другой crate в workspace (если `serde` нужен и `kernel`, и `gateway` — объединяются все features обоих).

**Аналогия в C++:** как `#ifdef FEATURE_X` блоки в коде библиотеки + CMake `option(FEATURE_X)`. Но в Rust это первоклассная сущность системы сборки, а не ручные `#define`.

---

## Сводка: привычки C++, которые нужно «переучить»

| Привычка C++ | В Rust | Почему |
|--------------|--------|--------|
| `auto x = y;` копирует | `let x = y;` **перемещает** | Ownership. Копия — только `Clone` |
| Всё мутабельно по умолчанию | Всё **иммутабельно** по умолчанию (`let`) | Safety + оптимизации |
| `const T&` / `T&` явные | `&T` / `&mut T` с borrow checker | Компилятор **доказывает** отсутствие data race |
| `std::string` создаётся неявно из `"literal"` | `"literal".to_string()` — **явная** аллокация | Никаких скрытых аллокаций |
| `v[i]` — UB при out-of-bounds | `v[i]` — **паника** (safe) | Безопасность по умолчанию |
| `int` → `long` неявно | `i32` → `i64` только через `from`/`as` | Никаких неявных конверсий |
| CMake + Conan + CTest + Doxygen | `cargo` = всё-в-одном | Единая экосистема |
| `try/catch` exceptions | `Result<T, E>` + `?` оператор | Ошибки — значения, не исключения |
| Dangling pointer = UB в runtime | Dangling reference = **ошибка компиляции** | Borrow checker |
| `std::move()` — подсказка компилятору | Move — **единственный** способ передачи (для не-Copy типов) | Ownership |
