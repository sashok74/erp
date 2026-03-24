# PostgreSQL — подключение к БД

## Параметры

| Параметр | Значение |
|----------|----------|
| Хост | 192.168.7.123 (LXC postgres, VMID 113) |
| Порт | 5432 |
| База данных | erp_dev |
| Пользователь | erp_admin |
| Пароль | vfrfh123 |
| Расширения | uuid-ossp |
| Версия | PostgreSQL 15.15 |
| Аутентификация | scram-sha-256 |

## DATABASE_URL

```
postgres://erp_admin:vfrfh123@192.168.7.123:5432/erp_dev
```

## Подключение вручную

```bash
psql -h 192.168.7.123 -U erp_admin -d erp_dev
```

## .env

Файл `/home/raa/projects/.env`:
```
DATABASE_URL=postgres://erp_admin:vfrfh123@192.168.7.123:5432/erp_dev
```

---

## Шаги создания БД

Все команды выполнялись на хосте postgres (192.168.7.123) под суперпользователем postgres.

### 1. Создание роли

```sql
CREATE ROLE erp_admin WITH LOGIN PASSWORD vfrfh123;
```

### 2. Создание базы данных

```sql
CREATE DATABASE erp_dev OWNER erp_admin;
```

### 3. Установка расширения uuid-ossp

```sql
\c erp_dev
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
```

### 4. Права на создание схем

```sql
GRANT CREATE ON DATABASE erp_dev TO erp_admin;
```

### 5. Проверка pg_hba.conf

Файл `/etc/postgresql/15/main/pg_hba.conf` уже содержал правило для локальной сети:

```
host    all     all     192.168.7.0/24      scram-sha-256
```

Дополнительных изменений не потребовалось.

### 6. Проверка подключения с erp-dev (192.168.7.128)

```bash
PGPASSWORD=vfrfh123 psql -h 192.168.7.123 -U erp_admin -d erp_dev -c "SELECT version()"
```

Результат: PostgreSQL 15.15, расширение uuid-ossp установлено.
