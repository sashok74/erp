# Rust-ликбез для C++ программиста — Синтаксические конструкции из проекта ERP

> Все примеры взяты из реального кода проекта (`crates/kernel/`, `crates/gateway/`).
> Каждая конструкция объясняется через аналогии с C++.

---

## 1. Атрибуты уровня crate: `#![...]`

```rust
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `#!` | Атрибут, применяемый ко **всему crate** (а не к следующему элементу). `!` означает «inner attribute» — действует изнутри | Ближайшее — `#pragma` на весь translation unit |
| `[warn(...)]` | Директива компилятору: выдавать **предупреждения** при срабатывании указанного lint'а | `-Wextra` в GCC/Clang |
| `clippy::pedantic` | Группа lint'ов из Clippy — «педантичные» проверки стиля и возможных ошибок (~100 правил) | `-Wpedantic -Weffc++` |
| `[allow(...)]` | Подавить предупреждение для указанного lint'а | `#pragma GCC diagnostic ignored "-W..."` |
| `clippy::module_name_repetitions` | Ругается, когда имя типа повторяет имя модуля (напр. `types::TypeId`). В DDD-стиле это норма | Нет аналога — в C++ пространства имён и имена классов часто пересекаются |

**Зачем в ERP:** включаем строгие проверки (как `-Wall -Wextra -Werror`), но отключаем те, которые мешают DDD-нейминг конвенциям.

---

## 2. Документирующие комментарии: `//!` и `///`

```rust
//! ERP Kernel — Platform SDK для Bounded Contexts.
//!
//! Определяет контракты (трейты), идентификаторы, ошибки и формат событий.

/// Идентификатор tenant'а (арендатора).
///
/// Каждый tenant — изолированная организация в мультитенантной ERP.
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `//!` | Doc-комментарий **модуля/crate** (inner doc). Описывает файл, в котором написан | Doxygen `/** @file ... */` в начале `.h`-файла |
| `///` | Doc-комментарий **следующего элемента** (outer doc). Описывает struct, fn, trait и т.д. | Doxygen `/** ... */` перед классом/функцией |

Оба поддерживают **Markdown**. Генерируют HTML-документацию через `cargo doc` (аналог `doxygen`).

---

## 3. Объявление модулей и реэкспорт: `pub mod`, `pub use`

```rust
pub mod commands;
pub mod entity;
pub mod errors;
pub mod events;
pub mod types;

pub use commands::{Command, CommandEnvelope};
pub use types::{EntityId, RequestContext, TenantId, UserId};
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `pub` | Модификатор видимости — делает элемент **публичным** | `public:` секция в классе. Но тут — для модуля, не класса |
| `mod commands;` | Объявляет подмодуль `commands`. Rust ищет файл `commands.rs` | `#include "commands.h"` — но это **не текстовая вставка**, а настоящая модульная система (ближе к C++20 modules) |
| `pub mod` | Подмодуль, доступный **извне crate** | `export module` в C++20 |
| `pub use` | **Реэкспорт** — пробрасывает тип наружу без необходимости лезть в подмодуль | `using kernel::types::TenantId;` в namespace + `export` |
| `{Command, CommandEnvelope}` | Множественный импорт из одного модуля | Нет прямого аналога; в C++ пишут отдельный `using` на каждое имя |

**Зачем в ERP:** позволяет писать `use kernel::TenantId` вместо `use kernel::types::TenantId` — как если бы ты сделал `using namespace` для избранных имён (но точечно, без загрязнения).

---

## 4. Импорт: `use`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::types::{RequestContext, TenantId, UserId};
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `use` | Импорт типов/функций из других модулей | `using chrono::DateTime;` (C++20 modules) или `#include` + `using` |
| `chrono::{DateTime, Utc}` | Из crate `chrono` импортируем два типа. Фигурные скобки — multiple import | Два `using`-объявления |
| `std::fmt` | Из стандартной библиотеки импортируем модуль `fmt` (форматирование) | `#include <format>` (C++20) |
| `crate::types::...` | `crate` — корень текущего crate | Как `::myproject::types::...` — полный путь от корня namespace |

**Ключевое отличие от C++:** `use` не копирует текст — это настоящий символьный импорт. Нет header guards, нет include-order проблем, нет ODR-нарушений.

---

## 5. Derive-макросы: `#[derive(...)]`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TenantId(Uuid);
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `#[derive(...)]` | Процедурный макрос — автоматически генерирует реализации trait'ов | **Нет прямого аналога.** В C++ ты бы писал всё руками: `operator==`, `operator<<`, copy constructor. Ближайшее — C++20 `operator<=>` = default, но только для сравнения |
| `Debug` | Вывод через `{:?}` (отладочный формат) | `operator<<(ostream&)` для отладки |
| `Clone` | Метод `.clone()` — создать глубокую копию | Copy constructor: `T(const T&)` |
| `Copy` | Тип **копируется неявно** при передаче, как `int` | Тривиально копируемый тип (`std::is_trivially_copyable`). В C++ все POD-типы и так копируются, но в Rust **по умолчанию — перемещение** (как если бы в C++ все типы были move-only) |
| `PartialEq` | Оператор `==` | `operator==(const T&) const` |
| `Eq` | Маркер: `a == a` всегда `true` | Нет отдельного аналога (в C++ `NaN != NaN` — просто так) |
| `Hash` | Можно использовать как ключ в `HashMap` | `std::hash<T>` специализация |
| `Serialize` / `Deserialize` | Из crate `serde` — (де)сериализация в JSON и т.д. | Нет стандартного аналога. Ближе всего — protobuf/Boost.Serialization, но тут одна аннотация на всё |
| `sqlx::Type` | Тип можно использовать в SQL-запросах | Нет аналога |
| `#[sqlx(transparent)]` | «Это обёртка над одним типом — используй внутренний тип напрямую» | Как если бы ORM знала, что `TenantId` — это просто `UUID` внутри |

**Главная мысль:** в C++ ты пишешь 5 специальных функций-членов (Rule of Five). В Rust — одна строка `#[derive(...)]`. Компилятор генерирует корректный код на этапе компиляции.

---

## 6. Newtype-паттерн: tuple struct

```rust
pub struct TenantId(Uuid);
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `pub struct` | Публичная структура | `struct` (в C++ struct = class с public по умолчанию) |
| `TenantId` | Имя нового типа | |
| `(Uuid)` | **Tuple struct** — структура с одним безымянным полем. Это «newtype» | `struct TenantId { private: Uuid value; };` — но без boilerplate |

Поле **приватное** (нет `pub` перед `Uuid`), поэтому извне нельзя написать `TenantId(some_uuid)` — только через `TenantId::new()` или `TenantId::from_uuid()`. Как `private:` конструктор + `static` фабричный метод в C++.

**Зачем в ERP:** компилятор запрещает передать `UserId` туда, где ожидается `TenantId`, хотя оба — UUID. В C++ это `strong typedef` / `NamedType<>`, в Rust — встроено в язык, zero-cost (никакого runtime overhead).

---

## 7. Блок `impl` — методы и ассоциированные функции

```rust
impl TenantId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `impl TenantId` | Блок реализации — методы и ассоциированные функции | Тело класса или определения методов вне класса (`TenantId::new()`) |
| `#[must_use]` | Компилятор предупредит, если возвращаемое значение проигнорировано | `[[nodiscard]]` (C++17) |
| `pub fn new() -> Self` | **Ассоциированная функция** (нет `self` в параметрах). Вызов: `TenantId::new()` | `static TenantId create()` — статический метод класса |
| `pub fn as_uuid(&self) -> &Uuid` | **Метод** (есть `&self`). Вызов: `id.as_uuid()` | `const Uuid& as_uuid() const` — const метод, возвращающий const ref |
| `&self` | Неизменяемая ссылка на экземпляр | Неявный `const this->` |
| `self.0` | Доступ к первому полю tuple struct | `this->value` (если бы поле называлось `value`) |
| `&self.0` | Возвращаем **ссылку**, не копию | `return this->value;` где return type — `const Uuid&` |
| `Self` | Псевдоним текущего типа внутри `impl` | Нет прямого аналога; C++ позволяет просто писать имя класса |

**Ключевое отличие от C++:** в Rust `self` — **явный первый параметр**, а не скрытый `this`. Это делает ownership/borrowing очевидным: `self` (забирает владение), `&self` (берёт const ref), `&mut self` (берёт mut ref).

---

## 8. Реализация trait'а для типа: `impl Trait for Type`

```rust
impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `impl Default for TenantId` | Реализуем стандартный trait `Default` | Конструктор по умолчанию `TenantId()` |
| `impl fmt::Display for TenantId` | Trait для человекочитаемого вывода (`{}`, `.to_string()`) | `friend ostream& operator<<(ostream&, const TenantId&)` |
| `f: &mut fmt::Formatter<'_>` | Буфер форматирования. `&mut` — изменяемая ссылка | `ostream& os` — неконстантная ссылка на поток |
| `'_` | Elided lifetime — компилятор выведет время жизни сам | Нет аналога — в C++ ссылки не имеют явных lifetime'ов |
| `fmt::Result` | `Result<(), fmt::Error>` | Функция может вернуть ошибку; в C++ `operator<<` обычно не сообщает об ошибках |
| `write!(f, "{}", self.0)` | Макрос — записывает форматированный текст в `f` | `os << this->value` |

**Ключевое отличие от C++:** trait'ы — это как абстрактные классы (pure virtual), но реализуются **снаружи типа** через `impl Trait for Type`. Можно реализовать чужой trait для своего типа или свой trait для чужого типа (в C++ пришлось бы наследовать или писать ADL-функцию).

---

## 9. Структура с именованными полями

```rust
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub timestamp: DateTime<Utc>,
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `pub struct RequestContext { ... }` | Публичная структура с именованными полями | `struct RequestContext { ... };` (не забудь `;` — в Rust не нужна!) |
| `pub tenant_id: TenantId` | Публичное поле. `pub` **на каждом поле** — по умолчанию поля приватны | `public: TenantId tenant_id;` |
| `DateTime<Utc>` | Параметризованный тип | `DateTime<Utc>` — шаблон (template), синтаксис одинаковый |

**Отличие от C++:** в Rust `struct` без `pub` перед полями = все поля приватные (как `class` в C++). В C++ `struct` = по умолчанию `public`.

---

## 10. Инициализация struct (field init shorthand)

```rust
Self {
    tenant_id,
    user_id,
    correlation_id: Uuid::now_v7(),
    causation_id: Uuid::now_v7(),
    timestamp: Utc::now(),
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `tenant_id,` | **Field init shorthand** — если переменная называется так же, как поле, можно не писать `tenant_id: tenant_id` | В C++20 designated initializers: `.tenant_id = tenant_id`, но без shorthand |
| `correlation_id: Uuid::now_v7()` | Полная форма — `поле: значение` | `.correlation_id = Uuid::now_v7()` |

**Отличие от C++:** в Rust нет конструкторов как таковых. Структуры инициализируются **перечислением полей**. Нет проблемы с порядком инициализации членов (C++ initializer list order).

---

## 11. Trait (интерфейс): определение

```rust
pub trait DomainEvent: Serialize + Send + Sync + 'static {
    fn event_type(&self) -> &'static str;
    fn aggregate_id(&self) -> Uuid;
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `pub trait DomainEvent` | Определение trait'а | `class DomainEvent` с чисто виртуальными методами (abstract class / interface) |
| `: Serialize + Send + Sync + 'static` | **Supertrait bounds** — «наследование» от нескольких trait'ов | Множественное наследование: `class DomainEvent : public Serialize, public Send, ...` |
| `Send` | Тип можно передать в другой поток | Нет аналога — в C++ любой тип можно передать в `std::thread`, и если что-то пойдёт не так, это UB. Rust проверяет **на этапе компиляции** |
| `Sync` | `&T` можно разделять между потоками | Аналогично — в C++ это ответственность программиста |
| `'static` | Тип не содержит временных ссылок | Гарантирует, что объект можно сохранить «навечно» (в `HashMap`, в async task). В C++ ты просто надеешься, что указатель валиден |
| `fn event_type(&self) -> &'static str` | Метод без реализации (обязателен к реализации) | `virtual const char* event_type() const = 0;` — чисто виртуальная функция |
| `&'static str` | Строковый срез, живущий всю программу | `const char*` на строковый литерал (он в `.rodata`, живёт вечно) |

**Ключевое отличие от C++:** trait'ы — **не наследование**, а контракт. Нет vtable по умолчанию. При использовании через generic (`<T: DomainEvent>`) — **мономорфизация** (как C++ templates). Виртуальная диспетчеризация (`dyn DomainEvent`) — только если попросишь явно.

---

## 12. Дженерики (Generics) и trait bounds

```rust
pub struct CloudEvent<T: Serialize> {
    pub data: T,
    // ...
}

impl<T: Serialize> CloudEvent<T> {
    pub fn new(source: &str, event_type: &str, ..., data: T, ctx: &RequestContext) -> Self { ... }
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `<T: Serialize>` | Параметр типа `T` с ограничением: `T` обязан реализовать `Serialize` | `template<Serializable T>` (C++20 concepts). Без concepts — `template<typename T>` и SFINAE / `static_assert` |
| `pub data: T` | Поле с generic-типом | `T data;` в template class |
| `impl<T: Serialize> CloudEvent<T>` | Методы определены для всех `CloudEvent<T>`, где `T: Serialize` | `template<Serializable T> void CloudEvent<T>::new(...)` |
| `source: &str` | Строковый срез — ссылка на строковые данные, не владеет ими | `std::string_view` (C++17) — точная аналогия |
| `.to_string()` | Создаёт владеющую `String` из `&str` | `std::string(sv)` — создаёт `std::string` из `string_view` |

**Ключевое отличие от C++:** в C++ templates ошибки проявляются **в месте инстанцирования** (жуткие простыни ошибок). В Rust trait bounds проверяются **в месте определения** — ошибка будет чёткой: «тип `Foo` не реализует `Serialize`». Это как C++20 concepts, но обязательно и повсеместно.

---

## 13. Serde-атрибут: `#[serde(rename = "...")]`

```rust
#[serde(rename = "type")]
pub event_type: String,
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `#[serde(rename = "type")]` | При (де)сериализации поле называется `"type"` в JSON | Аннотации в protobuf/nlohmann_json — `NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE(...)` с маппингом |

Нужно потому, что `type` — зарезервированное слово в Rust, нельзя назвать поле `type`. В C++ `type` — не keyword, такой проблемы нет.

---

## 14. `Option<T>` — может быть, а может не быть

```rust
pub subject: Option<String>,
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `Option<String>` | Два варианта: `Some("value".to_string())` или `None` | `std::optional<std::string>` (C++17) — прямая аналогия |

**Ключевое отличие от C++:** в C++ можно забыть проверить `has_value()` и получить UB при `*opt`. В Rust компилятор **заставляет** обработать оба случая — `Some` и `None`. Нет null, нет `nullptr`, нет UB.

---

## 15. Ручная реализация `Deserialize`: lifetime + where clause

```rust
impl<'de, T: Serialize + DeserializeOwned> Deserialize<'de> for CloudEvent<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `impl<'de, T: ...>` | Два параметра: lifetime `'de` и тип `T` | `template<typename T>` — но `'de` нет аналога в C++ |
| `'de` | **Lifetime** — время жизни данных, из которых десериализуем | В C++ ты просто «знаешь», что JSON-строка жива, пока ты её парсишь. Rust это **доказывает на этапе компиляции** |
| `DeserializeOwned` | «Умеет десериализоваться, владея данными» (без заимствования из источника) | Парсер создаёт `std::string`, а не `string_view` в результат |
| `fn deserialize<D>` | Метод с **ещё одним** generic-параметром `D` | `template<typename D> static CloudEvent deserialize(D deserializer)` |
| `where D: serde::Deserializer<'de>` | `where`-clause — constraints вынесены отдельно для читаемости | C++20: `requires Deserializer<D>` или `requires` clause |
| `Result<Self, D::Error>` | Возвращает или готовый объект, или ошибку | `std::expected<CloudEvent, Error>` (C++23), или `std::variant<CloudEvent, Error>` |

**Lifetime — главная концепция без аналога в C++.** Это как если бы компилятор C++ *статически доказывал*, что каждый `const T&` и `T*` валидны на момент использования. Dangling reference → ошибка компиляции, а не UB.

---

## 16. Вложенная struct (helper pattern)

```rust
#[derive(Deserialize)]
struct CloudEventHelper<T> {
    specversion: String,
    // ...
    data: T,
}

let helper = CloudEventHelper::<T>::deserialize(deserializer)?;
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| Вложенная struct | Определена **внутри функции** — видна только в ней | Локальный struct внутри функции (C++ тоже позволяет, но редко используется) |
| `CloudEventHelper::<T>` | **Turbofish** `::< >` — явно указываем тип generic'а | `CloudEventHelper<T>::deserialize(...)` — в C++ аналогичный синтаксис, но без `::` перед `<` |
| `?` | Если ошибка — сразу вернуть наверх | См. пункт 17 |

**Зачем turbofish:** в C++ компилятор часто выводит `<T>` из аргументов. В Rust тоже — но когда не может, приходится подсказывать через `::<T>`. Двоеточие перед `<` нужно, чтобы парсер не путал `<` с оператором «меньше».

---

## 17. Оператор `?` — ранний возврат ошибки

```rust
let helper = CloudEventHelper::<T>::deserialize(deserializer)?;
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `?` | Если `Result` = `Err(e)` → `return Err(e)`. Если `Ok(v)` → извлечь `v` | Нет прямого аналога. Ближайшее — Boost.Outcome `BOOST_OUTCOME_TRY(v, expr)` или макрос `TRY(expr)` |

В C++ для обработки ошибок используют exceptions (`try/catch`) или коды возврата. Rust **не имеет exceptions** — вместо этого `Result<T, E>` + оператор `?`. Это как если бы в C++ каждая функция возвращала `std::expected<T, E>`, а `?` — встроенный сахар для `if (!result) return unexpected(result.error())`.

---

## 18. Trait с ассоциированным типом

```rust
pub trait AggregateRoot: Send + Sync {
    type Event: DomainEvent;

    fn apply(&mut self, event: &Self::Event);
    fn take_events(&mut self) -> Vec<Self::Event>;
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `type Event: DomainEvent` | **Ассоциированный тип** — каждая реализация определяет свой тип | `using Event = ...;` внутри класса. Ближайшее — `std::iterator_traits<I>::value_type` |
| `: DomainEvent` | Ограничение на ассоциированный тип | `static_assert(std::derived_from<Event, DomainEvent>)` или concept |
| `&mut self` | **Изменяемая** ссылка — метод модифицирует объект | Неконстантный метод: `void apply(const Event& e)` (без `const` на методе) |
| `&Self::Event` | Ссылка на событие конкретного типа | `const Event&` |
| `Vec<Self::Event>` | Вектор (динамический массив) | `std::vector<Event>` |

**Отличие от C++ templates:** ассоциированный тип — это **одно** конкретное определение типа на реализацию. В C++ — это `typedef`/`using` внутри класса. Но в Rust компилятор **проверяет ограничения** (bounds) на ассоциированный тип в месте определения trait, а не при инстанцировании.

---

## 19. `std::mem::take` — забрать и заменить пустым

```rust
fn take_events(&mut self) -> Vec<Self::Event> {
    std::mem::take(&mut self.events)
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `std::mem::take` | Забирает значение, оставляя на его месте `Default` (пустой вектор) | `std::exchange(this->events, {})` — забирает значение, подставляет пустой вектор. Или `std::move(this->events)` + ручной reset |

**Зачем:** Rust **запрещает** иметь объект в «moved-from» неопределённом состоянии (в отличие от C++, где после move объект в «valid but unspecified state»). `take` гарантирует: старое значение перемещено в return, на его месте — `Default`.

---

## 20. Enum с данными (алгебраический тип)

```rust
#[derive(Debug, Clone, Error)]
pub enum DomainError {
    #[error("Недостаточно остатков: требуется {required}, доступно {available}")]
    InsufficientStock { required: String, available: String },

    #[error("Баланс не может быть отрицательным")]
    NegativeBalance,

    #[error("Не найдено: {0}")]
    NotFound(String),

    #[error("Конфликт версий: ожидалась {expected}, получена {actual}")]
    ConcurrencyConflict { expected: i64, actual: i64 },
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `pub enum DomainError` | Тип, который может быть **одним из** вариантов | `std::variant<InsufficientStock, NegativeBalance, NotFound, ConcurrencyConflict>` — но с **именем варианта** и встроенным pattern matching |
| `InsufficientStock { required, available }` | Вариант с **именованными полями** (struct variant) | `struct InsufficientStock { string required; string available; };` внутри variant |
| `NegativeBalance` | Вариант **без данных** | Обычный `enum` value |
| `NotFound(String)` | Вариант с **безымянным полем** (tuple variant) | Один элемент в `variant` |
| `#[derive(Error)]` | Автоматически реализует `std::error::Error` | Нет аналога; в C++ наследуешь от `std::exception` вручную |
| `#[error("...{required}...")]` | Шаблон для `Display`. `{required}` подставляет поле | `what()` с `std::format(...)` |

**Ключевое отличие от C++:** в C++ `enum` — это просто числа. `std::variant` — ближе, но синтаксис громоздкий. Rust enum — tagged union с компактным синтаксисом и **компилятор заставляет обработать все варианты** при `match`.

---

## 21. `#[from]` — автоматическая конвертация ошибок

```rust
pub enum AppError {
    #[error("{0}")]
    Domain(#[from] DomainError),
    // ...
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `#[from]` | Генерирует `impl From<DomainError> for AppError` | Неявный converting constructor: `AppError(DomainError err)` без `explicit` |
| Эффект | `?`-оператор автоматически оборачивает `DomainError` → `AppError::Domain(...)` | Неявная конвертация при return |

```rust
fn fallible() -> Result<(), AppError> {
    Err(DomainError::NegativeBalance)?  // автоматически → AppError::Domain(NegativeBalance)
}
```

В C++ аналог: функция возвращает `expected<void, AppError>`, а `DomainError` неявно конвертируется в `AppError` через converting constructor.

---

## 22. Trait bounds для потокобезопасности: `Command: Send + Sync + 'static`

```rust
pub trait Command: Send + Sync + 'static {
    fn command_name(&self) -> &'static str;
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `Send + Sync + 'static` | Гарантии потокобезопасности, проверяемые **компилятором** | В C++ — просто пишешь в документации «thread-safe». Или `static_assert(std::is_trivially_copyable_v<T>)` — но это покрывает малую долю |
| `&'static str` | Строковый срез, живущий всю программу | `constexpr const char*` или строковый литерал |

**Ключевое отличие:** в C++ data race = UB, и компилятор **не помогает** его избежать. В Rust `Send`/`Sync` — compile-time proof, что тип безопасен для многопоточности. Это одна из главных причин, почему Rust выбирают для системного программирования.

---

## 23. Generic struct: `CommandEnvelope<C: Command>`

```rust
#[derive(Debug)]
pub struct CommandEnvelope<C: Command> {
    pub command: C,
    pub context: RequestContext,
}

impl<C: Command> CommandEnvelope<C> {
    #[must_use]
    pub fn new(command: C, context: RequestContext) -> Self {
        Self { command, context }
    }
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `<C: Command>` | Тип-параметр `C` с ограничением | `template<Command C>` (C++20 concept) или `template<typename C>` + `static_assert` |
| `impl<C: Command>` | Методы для всех `CommandEnvelope<C>` | `template<Command C> CommandEnvelope<C>::new(...)` |
| `Self { command, context }` | Конструктор — инициализация полей | Designated initializers / aggregate init: `{.command = cmd, .context = ctx}` |

Как и в C++ templates, каждый `CommandEnvelope<ConcreteCommand>` — **отдельный тип** после мономорфизации. Zero-cost abstraction.

---

## 24. Тесты: `#[cfg(test)]`, `#[test]`, `assert!`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_id_new_returns_valid_uuid_v7() {
        let id = TenantId::new();
        assert_eq!(id.as_uuid().get_version_num(), 7);
    }
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `#[cfg(test)]` | **Условная компиляция** — только при запуске тестов | `#ifdef TESTING` / `#if defined(TESTING)` |
| `mod tests` | Вложенный модуль для тестов | Отдельный `.cpp`-файл с тестами (gtest) |
| `use super::*` | Импорт всего из родительского модуля | Тесты подключают header тестируемого модуля |
| `#[test]` | Помечает функцию как тест | `TEST(Suite, Name) { ... }` в Google Test |
| `assert_eq!(a, b)` | Паника, если `a != b` | `ASSERT_EQ(a, b)` в gtest |
| `assert!(expr)` | Паника, если `false` | `ASSERT_TRUE(expr)` |

**Отличие от C++:** тесты живут **рядом с кодом** в том же файле, а не в отдельном test-binary. `#[cfg(test)]` гарантирует, что тестовый код **не попадёт в production-сборку**. Не нужен отдельный framework — `cargo test` встроен.

---

## 25. `let`-привязки, `.unwrap()`, type inference

```rust
let id = TenantId::new();
let json = serde_json::to_string(&id).unwrap();
let deserialized: TenantId = serde_json::from_str(&json).unwrap();
assert_eq!(id, deserialized);
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `let id = ...` | Привязка переменной. Тип выводится автоматически | `auto id = ...;` (C++11) |
| `let deserialized: TenantId = ...` | Явная аннотация типа | `TenantId deserialized = ...;` без `auto` |
| `.unwrap()` | Извлекает `Ok`/`Some`. **Паникует при ошибке** | `result.value()` у `std::optional`/`std::expected` — тоже бросает, но exception. `.unwrap()` — паника (abort-like) |
| `&id` | Передаём ссылку, не владение | Передаём `const TenantId&` |

**Ключевое отличие:** `let` в Rust = **immutable** по умолчанию (как `const auto` в C++). Для изменяемой переменной нужен `let mut` (как `auto` без `const`). В C++ — наоборот: `auto` мутабельно, `const auto` — нет.

---

## 26. `matches!` — паттерн-матчинг в одну строку

```rust
assert!(matches!(
    err,
    AppError::Domain(DomainError::NegativeBalance)
));

assert!(matches!(downcasted, DomainError::NotFound(msg) if msg == "Item #42"));
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `matches!(expr, pattern)` | `true`, если `expr` соответствует `pattern` | `std::holds_alternative<T>(variant)` — но без вложенного matching |
| `AppError::Domain(DomainError::NegativeBalance)` | **Вложенный** паттерн | Нет аналога — в C++ `std::visit` с лямбдой, многословно |
| `if msg == "Item #42"` | **Guard** — дополнительное условие | `if` внутри `std::visit` |

**Ключевое отличие:** pattern matching в Rust — **first-class feature**. В C++ `std::variant` + `std::visit` + лямбды — функциональный, но синтаксически громоздкий аналог.

---

## 27. Downcasting: `.source()` и `.downcast_ref()`

```rust
let source = app_err.source().expect("source should be present");
let downcasted = source.downcast_ref::<DomainError>().unwrap();
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `.source()` | Возвращает причину ошибки (`Option<&dyn Error>`) | `std::exception::what()` не хранит причину; ближе — вложенный `std::nested_exception` |
| `.expect("msg")` | Как `.unwrap()`, но с сообщением | `value_or_throw("msg")` |
| `dyn Error` | **Trait object** — стёртый тип, известен только интерфейс | `std::exception&` — ссылка на базовый класс (dynamic dispatch через vtable) |
| `.downcast_ref::<DomainError>()` | Привести `&dyn Error` к `&DomainError` | `dynamic_cast<const DomainError*>(&err)` — прямая аналогия! |
| `::<DomainError>` | Turbofish — явно указываем целевой тип | `<DomainError>` в `dynamic_cast` |

`dyn Trait` в Rust = полиморфизм через vtable, как в C++. Но `dyn` **явный** — ты видишь, где runtime dispatch.

---

## 28. Dereference: `*self.id.as_uuid()`

```rust
let event = Incremented {
    value,
    aggregate: *self.id.as_uuid(),
};
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `*` | Разыменование — `as_uuid()` возвращает `&Uuid`, `*` копирует в `Uuid` (т.к. `Uuid: Copy`) | `*ptr` — разыменование. Если тип trivially copyable — копия; аналогия точная |

В C++ `*ref` для ссылки не нужен — ссылка и так ведёт себя как значение. В Rust `&T` и `T` — **разные типы**, `*` превращает одно в другое явно.

---

## 29. `fn main()` — точка входа

```rust
fn main() {
    println!("ERP Gateway — placeholder");
}
```

| Элемент | Значение | Аналог в C++ |
|---------|----------|--------------|
| `fn main()` | Точка входа. Без `pub` — вызывается рантаймом | `int main()` — один в один |
| `println!` | Макрос для печати в stdout с `\n` | `std::println("{}", ...)` (C++23) или `std::cout << ... << '\n'` |

**Отличие:** в Rust `main()` не возвращает int. Может возвращать `Result<(), Error>` для обработки ошибок при старте.

---

## Сводная таблица: Rust → C++ маппинг

| Rust | C++ | Комментарий |
|------|-----|-------------|
| `::` | `::` | Одинаковый смысл: путь к вложенному имени |
| `&` | `const T&` | Неизменяемая ссылка. В Rust — по умолчанию |
| `&mut` | `T&` | Изменяемая ссылка |
| `*ref` | `*ptr` | Разыменование |
| `?` | `TRY()` / exceptions | Ранний возврат ошибки |
| `!` после имени | — | Вызов макроса (нет аналога в синтаксисе) |
| `#![...]` | `#pragma` | Inner attribute |
| `#[...]` | `[[...]]` (C++11 attributes) | Outer attribute |
| `<T>` | `template<typename T>` | Generics / шаблоны |
| `<T: Bound>` | `template<Concept T>` (C++20) | Constrained generics |
| `'a`, `'de` | — | Lifetimes. **Нет аналога в C++** — это compile-time proof, что ссылки валидны |
| `'static` | `constexpr` / глобальный | Живёт всю программу |
| `Self` | имя класса | Псевдоним текущего типа внутри `impl` |
| `self` | `*this` | Явный первый параметр (в отличие от скрытого `this`) |
| `crate` | `::mylib` | Корень текущего crate |
| `super` | parent namespace | Родительский модуль |
| `let` | `const auto` | Immutable по умолчанию! |
| `let mut` | `auto` | Mutable |
| `Vec<T>` | `std::vector<T>` | Динамический массив |
| `String` | `std::string` | Владеющая строка (heap) |
| `&str` | `std::string_view` | Не-владеющий срез строки |
| `Option<T>` | `std::optional<T>` | Может быть пусто |
| `Result<T, E>` | `std::expected<T, E>` (C++23) | Значение или ошибка |
| `enum` с данными | `std::variant` | Tagged union |
| `trait` | abstract class / concept | Контракт (интерфейс) |
| `impl Trait for T` | наследование / specialization | Реализация контракта |
| `dyn Trait` | `Base&` / `Base*` | Динамическая диспетчеризация (vtable) |
| `derive` | — | Автогенерация (Rule of Five бесплатно) |
| `mod` | `namespace` + `#include` | Модульная система |
| `Send + Sync` | «thread-safe» (документация) | Compile-time thread safety |
