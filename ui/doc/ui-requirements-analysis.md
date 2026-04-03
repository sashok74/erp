# UI Requirements Analysis: MERP ERP → New Web ERP

**Дата**: 2026-04-02
**Источник**: Функциональная декомпозиция desktop MERP (C++ MFC)
**Цель**: Определить UI-компоненты, interaction-паттерны и системные требования для нового web ERP

---

## 1. Executive Summary (15 пунктов)

1. **Advanced Data Grid** — критический компонент №1. 100+ форм используют grid с inline editing, drag-drop, группировкой, контекстным меню, виртуализацией, 10+ типами ячеек. Стек ОБЯЗАН покрывать это на уровне enterprise grid.

2. **Document Workflow Engine** — сквозная система согласования (Concord) пронизывает 10+ типов документов. Нужен UI для маршрутов, шагов, статусных переходов, кабинета задач.

3. **Master-Detail Layout** — базовый паттерн ERP: список документов → карточка с вложенными гридами, вкладками, деревьями. ~60 форм реализуют этот паттерн.

4. **CRUD-формы с rich validation** — 214 диалогов с маппингом контролов на параметры, историей undo/redo, бизнес-валидацией, permission-aware поведением.

5. **Tree + Grid Hybrid** — 11+ форм используют дерево-категорий + таблицу-содержимого (DocApprove, Nomenclatura, Storage, Department, Specifications).

6. **Role-Based UI** — каждая форма проверяет привилегии (canEdit, canDelete, canAdd, CheckPrivilege). Кнопки, поля, целые секции скрываются/блокируются по ролям.

7. **Print/Export** — 58 отчётов (24 Excel + 34 HTML), формы КС-2, ТОРГ-12, М-15, накладные. Критично для бизнеса, стек должен поддерживать серверную генерацию документов.

8. **Lookup/Selector Pattern** — 28+ диалогов выбора (сотрудник, контрагент, номенклатура, склад, договор). В web это autocomplete + modal picker с фильтрацией.

9. **File Attachments** — 6+ модулей поддерживают вложения (DocApprove, Correspondence, Contracts, QualityControl, ProjectJobReport).

10. **Real-time Monitoring** — производственные линии (Modbus), статус-бар с таймингами БД, toast-уведомления, фоновые задачи.

11. **Gantt Chart** — управление проектами, производственный план, график ТОИР. Специализированная визуализация.

12. **Import/Export Wizards** — MS Project, Excel, XML, спецификации. Multi-step wizard с маппингом колонок.

13. **MDI Shell с контекстным ribbon** — 198 форм открываются как вкладки, ribbon меняется динамически. В web это SPA с tab-manager и context-aware toolbar.

14. **Keyboard-Heavy Interaction** — hotkeys, Tab-навигация по формам, быстрый поиск в ribbon, inline editing в гридах. ERP-пользователи работают с клавиатуры.

15. **Multi-Tenant + Offline** — новая система multi-tenant (JWT, tenant isolation). Offline-режим не критичен для текущего desktop, но может стать требованием для мобильных сценариев.

---

## 2. Карта модулей и экранов

| Модуль | Сценарий | Экран / рабочая область | Основная роль | Ключевые компоненты |
|--------|----------|------------------------|---------------|---------------------|
| **Склад** | Перемещение материалов | Список перемещений → Карточка перемещения | Кладовщик, МТО | DataGrid, CRUD-form, Lookup (склад, номенклатура, контрагент), StatusBar |
| **Склад** | Приходный ордер | Список ордеров → Карточка ордера | Кладовщик | DataGrid, CRUD-form, Lookup (поставщик, номенклатура), Print (M-15) |
| **Склад** | Списание материалов | Список списаний → Карточка списания | Кладовщик, Производство | DataGrid, CRUD-form, Lookup, Workflow (проведение/отмена) |
| **Склад** | Потребность в материалах | Список потребностей → Карточка потребности | Снабженец | DataGrid, CRUD-form, Workflow (approve/reject), Create-from (→SupplierOrder, →MoveMaterial) |
| **Склад** | Инвентаризация | Список → Карточка | Кладовщик | DataGrid, CRUD-form, Import (расчётные остатки) |
| **Склад** | Остатки по складу | Аналитический отчёт | Все | DataGrid (read-only), Filters, Export |
| **Склад** | Заказ поставщику | Список → Карточка | Снабженец | DataGrid, CRUD-form, Workflow, Print, Create-from (Needs) |
| **Производство** | Производственное задание | Список → Карточка | Мастер | DataGrid, CRUD-form, StatusWorkflow (5 статусов), Create-from (materials) |
| **Производство** | Производственный заказ | Список заказов → Карточка | Плановик | DataGrid, TreeView, CRUD-form, Batch operations, Excel export |
| **Производство** | План производства | Графический/табличный план | Плановик | Gantt, DataGrid, Filters, Export (Excel, HTML) |
| **Производство** | Мониторинг линий | Реальное время | Оператор | Real-time dashboard, Modbus, Error groups, Manual mode |
| **Производство** | Вспомогательное производство | Список → Карточка | Мастер | DataGrid, CRUD-form, StatusWorkflow |
| **Производство** | Отчёт выпуска продукции | 5 версий отчёта | Плановик, Руководство | DataGrid (read-only), Filters, Group, Export (Excel, HTML) |
| **Проекты** | Управление проектами | Список проектов → Gantt | PM | Gantt, DataGrid, CRUD-form, TreeView |
| **Проекты** | Выполнение работ (КС-2) | Список → Карточка отчёта | Прораб | DataGrid, CRUD-form, File attachments, Workflow, Print (КС-2) |
| **Проекты** | Монтаж | Монтажная ведомость | Прораб | DataGrid, Filters, Checkbox marking |
| **Проекты** | Списание по работам | Списания под отчёт | Прораб | DataGrid, Links to WriteOff |
| **Строительство** | Объекты строительства | Список → Карточка с модулями | PM, Инженер | TreeView, CRUD-form, Hierarchy (секция/этаж), Import/Export XML |
| **Договоры** | Договоры | Список → Карточка | Юрист, PM | DataGrid, CRUD-form, Tabs (реквизиты, спецификация, финансы), Workflow (Concord), File attachments |
| **Договоры** | Ценовой протокол | Список → Карточка | Экономист | DataGrid, CRUD-form, Workflow, Import/Export Excel |
| **Договоры** | Оплаты | Список → Карточка | Бухгалтер | DataGrid, CRUD-form, Workflow (Concord) |
| **Реализация** | Продажи | Список → Карточка | Менеджер | DataGrid, CRUD-form, Workflow (проведение), Print (ТОРГ-12, счёт) |
| **DocApprove** | Проектная документация | Каталог → Документ → Версии → Шаги → Замечания | ГИП, Эксперт | TreeView+Grid hybrid, Multi-tab form, StatusWorkflow (11 состояний), File attachments, Remarks, History |
| **DocApprove** | Поиск документов | Полнотекстовый поиск | Все | Search form, DataGrid results, Navigation |
| **СЭД** | Входящая корреспонденция | Список → Карточка | Делопроизводитель | DataGrid, CRUD-form, File attachments, Create-from (→PersonalInstruction) |
| **СЭД** | Исходящая корреспонденция | Список → Карточка | Делопроизводитель | DataGrid, CRUD-form, File attachments, Workflow |
| **СЭД** | Поручения | Список → Карточка | Руководитель | DataGrid, CRUD-form, Workflow (Concord), Linked docs |
| **СЭД** | Кабинет задач | Единый список задач | Все | DataGrid, StatusFilters, Quick actions (viewed/unviewed), Navigation to source doc |
| **Качество** | Задания КК | Список → Карточка | Контролёр | DataGrid, CRUD-form, StatusWorkflow (5 статусов), Defect catalog |
| **Качество** | Технадзор | Список → Карточка | Инспектор | DataGrid, CRUD-form, StatusWorkflow |
| **ТОИР** | Оборудование | Список/Дерево → Карточка | Механик | TreeView, CRUD-form, Linked materials/elements |
| **ТОИР** | Сервисные заявки | Список → Карточка | Механик | DataGrid, CRUD-form, StatusWorkflow, Create-from (→Need, →WriteOff) |
| **ТОИР** | График ТОИР | Визуализация | Гл. механик | Gantt/Timeline, Filters |
| **Заказы оборудования** | Заказ оборудованию | Список → Карточка | Инженер | DataGrid, CRUD-form, Barcode search, File attachments, Print |
| **Спецификации** | Спецификации | Список → Карточка | Конструктор | TreeView+Grid, CRUD-form, Mass operations (replace, copy, split), Workflow, Export |
| **Номенклатура** | Справочник номенклатуры | Дерево групп → Список | Все | TreeView+Grid, CRUD-form, Search, Parameters, Replacements |
| **Справочники** | Контрагенты | Список → Карточка | Все | DataGrid, CRUD-form, Address management |
| **Справочники** | Сотрудники | Список → Карточка | HR | DataGrid, CRUD-form, Role assignment |
| **Справочники** | Подразделения | Дерево → Карточка | HR | TreeView, CRUD-form, Hierarchy |
| **Справочники** | Склады | Дерево → Карточка | Администратор | TreeView+Grid, CRUD-form, Storage boxes |
| **Администрирование** | Роли и права | Список → Матрица прав | Админ | DataGrid, Permission matrix, CRUD-form |
| **Администрирование** | Пользователи | Список → Карточка | Админ | DataGrid, CRUD-form, Permission assignment |
| **Администрирование** | Глобальные настройки | Форма настроек | Админ | Form with sections, Key-value editing |
| **Навигация** | Дерево связей документов | Дерево | Все | TreeView, Cross-document navigation |
| **Файловый архив** | Архив документов | Файловый менеджер | Все | FileList, Preview (HTML), Upload/Download |

---

## 3. Каталог компонентов

### 3.1. Shell & Navigation

---

**Компонент**: Application Shell (MDI Container)
**Тип**: Shell / Layout
**Где используется**: Вся система
**Основная задача пользователя**: Работать с несколькими документами/формами одновременно, переключаться между ними
**Роли**: Все
**Что читает из системы**: Список открытых документов, состояние сессии, tenant context
**Что изменяет в системе**: Ничего (оркестрация)
**Команды / действия**: Открыть вкладку, закрыть вкладку, переключить вкладку, Ctrl+Tab
**Запросы / чтение**: Метаданные открытых форм
**Права доступа / ограничения**: Tenant isolation, session validation
**Валидация / бизнес-правила**: Подтверждение закрытия при unsaved changes
**Состояния компонента**: Authenticated / Loading / Ready / Session expired
**Связи с другими компонентами**: Navigation Bar, Context Toolbar, Status Bar, все ViewForms
**Требования к UX**: Tabs с иконками и кнопкой закрытия, drag-reorder tabs, tab overflow menu
**Объем данных / частота обновления**: ~5-15 одновременно открытых вкладок
**Признаки desktop-like поведения**: MDI tabs, context-sensitive toolbar, docking panels
**Признаки давления на стек**: basic CRUD UI (tabs), desktop-like interaction density (context ribbon)
**Приоритет**: must
**Уверенность**: high

**Историческая реализация**: MFC MDI с CMDIFrameWndEx, dockable panels (Properties, ClassView, Output). В web docking panels НЕ нужны — заменяются sidebar/drawer.
**Бизнес-требование**: Многооконная работа с документами. Пользователь держит 5-15 форм открытыми.
**Web-альтернатива**: SPA с tab bar + route-based navigation. Context toolbar обновляется при переключении tab.

---

**Компонент**: Navigation Bar (Outlook Bar)
**Тип**: Navigation
**Где используется**: Левая панель приложения
**Основная задача пользователя**: Быстро открыть нужный модуль/функцию
**Роли**: Все (видимость пунктов зависит от роли)
**Что читает из системы**: Functions (таблица БД), роли пользователя, счётчики задач
**Что изменяет в системе**: Ничего
**Команды / действия**: Клик по функции → открытие формы, drag-drop для favorites
**Запросы / чтение**: `SELECT * FROM Functions WHERE ParentID = ...`, task counters
**Права доступа / ограничения**: Пункты меню фильтруются по ролям. Role-based visibility.
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Collapsed / Expanded, Active section highlighted
**Связи с другими компонентами**: Shell (открывает вкладки), Task Monitor (badge counters)
**Требования к UX**: Иерархия (разделы → пункты), badge-счётчики, иконки, collapsible
**Объем данных / частота обновления**: ~50-100 пунктов меню, badge counters обновляются каждые 30-60 сек
**Признаки desktop-like поведения**: Outlook-style bar с drag-drop
**Признаки давления на стек**: basic CRUD UI
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Sidebar menu с collapsible sections, badge counters. Стандартный паттерн.

---

**Компонент**: Context Toolbar (Ribbon)
**Тип**: Toolbar
**Где используется**: Верхняя панель, меняется для каждой формы
**Основная задача пользователя**: Выполнить действие над текущим документом (создать, редактировать, удалить, провести, печать)
**Роли**: Все (кнопки enable/disable по ролям)
**Что читает из системы**: Тип текущей формы, состояние документа, привилегии пользователя
**Что изменяет в системе**: Инициирует команды (через формы)
**Команды / действия**: Add, Edit, Delete, Execute, Print, Export, Search, Custom per-form actions
**Запросы / чтение**: Привилегии пользователя
**Права доступа / ограничения**: Кнопки disabled/hidden по ролям и состоянию документа
**Валидация / бизнес-правила**: Кнопка "Провести" disabled если документ уже проведён
**Состояния компонента**: Кнопки: enabled/disabled/hidden. Набор кнопок зависит от активной формы.
**Связи с другими компонентами**: Shell, активная ViewForm, Permission system
**Требования к UX**: Иконки + текст, группировка по секциям, поисковая строка (RICHEDIT)
**Объем данных / частота обновления**: ~10-20 кнопок на форму, обновление при смене выделения в гриде
**Признаки desktop-like поведения**: MFC Ribbon с динамическими категориями
**Признаки давления на стек**: basic CRUD UI, complex permissions
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Page-level toolbar/action bar. Кнопки определяются компонентом страницы, а не глобальным ribbon.

---

**Компонент**: Status Bar
**Тип**: Feedback / Monitoring
**Где используется**: Нижняя панель приложения
**Основная задача пользователя**: Видеть статус подключения к БД, время выполнения запросов, уведомления
**Роли**: Все
**Что читает из системы**: Статус подключения, тайминги запросов, фоновые задачи
**Что изменяет в системе**: Ничего
**Команды / действия**: Нет (информационный)
**Запросы / чтение**: Database connection state, query timing
**Права доступа / ограничения**: Нет
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Connected (green) / Connecting (blue) / Disconnected (red) / Request in progress (orange)
**Связи с другими компонентами**: Database layer, Background tasks
**Требования к UX**: Цветовая индикация, real-time обновление
**Объем данных / частота обновления**: Continuous, при каждом запросе
**Признаки desktop-like поведения**: Persistent bottom bar с live metrics
**Признаки давления на стек**: real-time updates
**Приоритет**: should
**Уверенность**: medium

**Историческая реализация**: 6 panes в MFC CStatusBar с цветовым кодированием. Тайминги запросов — полезны для разработчиков, не для бизнес-пользователей.
**Web-альтернатива**: Минимальный status indicator (online/offline). Тайминги — в dev tools / admin panel.

---

### 3.2. Data Display & Editing

---

**Компонент**: DataGrid (Enterprise Grid)
**Тип**: Data Grid
**Где используется**: 100+ форм — списки документов, строки документов, отчёты, справочники
**Основная задача пользователя**: Просмотр, фильтрация, сортировка, выбор записей; inline-редактирование в строках документа
**Роли**: Все
**Что читает из системы**: SQL-запросы к таблицам (через *_SQL.h → API endpoints)
**Что изменяет в системе**: Inline edit → UPDATE строк, Drag-drop → перенос строк, Delete → удаление
**Команды / действия**: Select row, Double-click → open card, Context menu, Inline edit, Drag-drop, Copy to clipboard, Column resize, Sort, Group
**Запросы / чтение**: Параметризованные SQL через DataSet/GridDataSource
**Права доступа / ограничения**: canEdit, canDelete, canAdd per-row и per-form
**Валидация / бизнес-правила**: Cell-level validation (type, range, required), Row-level business rules
**Состояния компонента**: Loading, Empty, Populated, Editing (cell), Selected (row/range), Grouped, Filtered
**Связи с другими компонентами**: CRUD-form (double-click → open), Context Toolbar (actions on selection), Filters, Export
**Требования к UX**: Виртуализация (1000+ строк), быстрая сортировка, inline editing, drag-drop, column configuration, cell coloring, context menu, keyboard navigation (arrows, Enter, Tab, Escape)
**Объем данных / частота обновления**: 10-10000 строк, обновление при фильтрации / CRUD операциях
**Признаки desktop-like поведения**: 10+ типов ячеек (text, date, combo, checkbox, check-combo, button), drag-drop внутри и между гридами, cell coloring callbacks, title tips
**Признаки давления на стек**: advanced data grid, large dataset / virtualization, heavy keyboard navigation, desktop-like interaction density
**Приоритет**: must (КРИТИЧЕСКИЙ)
**Уверенность**: high

**Типы ячеек (из MGridCtrlEx)**:
- Text (free text entry)
- DateTime (with calendar picker)
- Combo (single-select dropdown)
- CheckCombo (multi-select dropdown)
- Checkbox
- Button (action trigger in cell)
- Numeric (with formatting)
- Read-only (display)
- Color-coded (background color by value)

**Бизнес-требование**: Grid — основной рабочий инструмент ERP. 80% времени пользователь работает в гриде.
**Web-альтернатива**: Enterprise grid library (AG Grid, TanStack Table + кастомизация, Handsontable). Это самый сложный компонент для web.

---

**Компонент**: TreeView (Hierarchical Tree)
**Тип**: Tree Control
**Где используется**: 11+ форм — DocApprove (каталог), Nomenclatura (группы), Storage (склады), Department (подразделения), Specifications, ConstructionObject (структура), TOIR Equipment
**Основная задача пользователя**: Навигация по иерархии, выбор узла для фильтрации связанного грида
**Роли**: Все
**Что читает из системы**: Иерархические данные (parent-child) из TreeDataSource
**Что изменяет в системе**: Выбор узла → фильтрация грида, CRUD для узлов дерева (add group, rename, delete, move)
**Команды / действия**: Expand/Collapse, Select, Add child, Rename, Delete, Drag-drop (reorder)
**Запросы / чтение**: Иерархические SQL (CTE или self-join)
**Права доступа / ограничения**: canEdit на уровне узла
**Валидация / бизнес-правила**: Запрет удаления непустых узлов, проверка циклов
**Состояния компонента**: Loading, Expanded, Collapsed, Selected, Editing (rename), DragOver
**Связи с другими компонентами**: DataGrid (master-detail: tree=master, grid=detail), CRUD-form
**Требования к UX**: Lazy loading для больших деревьев, multi-column tree (ColumnTreeCtrl), drag-drop, context menu, icons, expand all/collapse all
**Объем данных / частота обновления**: 50-5000 узлов, обновление при CRUD
**Признаки desktop-like поведения**: Multi-column tree (дерево с колонками), drag-drop nodes
**Признаки давления на стек**: advanced data grid (multi-column tree), desktop-like interaction density
**Приоритет**: must
**Уверенность**: high

**Историческая реализация**: CColumnTreeCtrl — дерево с дополнительными колонками данных. 32 колонки max.
**Web-альтернатива**: TreeGrid компонент или tree + adjacent grid. Multi-column tree — нишевый паттерн, может потребовать кастомизацию.

---

**Компонент**: Master-Detail Layout
**Тип**: Layout Pattern
**Где используется**: ~60 форм — Список документов → Карточка документа, Дерево категорий → Грид элементов
**Основная задача пользователя**: Навигация от списка к детальной карточке и обратно
**Роли**: Все
**Что читает из системы**: Список (grid) + детали (form) одного типа сущности
**Что изменяет в системе**: CRUD операции через детальную форму
**Команды / действия**: Select in list → load detail, Create new → empty detail, Save → update list
**Запросы / чтение**: List query + Detail query (by ID)
**Права доступа / ограничения**: По форме и по записи
**Валидация / бизнес-правила**: Валидация в detail form
**Состояния компонента**: List mode, Detail mode, Split mode (splitter)
**Связи с другими компонентами**: DataGrid (list), CRUD-form (detail), Context Toolbar
**Требования к UX**: Быстрое переключение list↔detail, сохранение позиции в списке при возврате, split view (resizable splitter)
**Объем данных / частота обновления**: Зависит от модуля
**Признаки desktop-like поведения**: MSplitterWnd — resizable split panels
**Признаки давления на стек**: rich forms, desktop-like interaction density
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Route-based (list page → detail page) или split layout с resizable panels. Оба паттерна стандартны для web.

---

**Компонент**: CRUD Form (Document Card)
**Тип**: Rich Form
**Где используется**: 214 диалогов — карточки документов, справочников, настроек
**Основная задача пользователя**: Создать / редактировать / просмотреть запись с полной валидацией
**Роли**: Все (поля read-only по ролям)
**Что читает из системы**: Одна запись + связанные данные (lookup-и), история изменений
**Что изменяет в системе**: INSERT / UPDATE одной записи + вложенных строк
**Команды / действия**: Save, Cancel, Delete, Undo/Redo, Print, Execute (провести), Create-from (создать на основании)
**Запросы / чтение**: SELECT одной записи, lookup queries для комбобоксов
**Права доступа / ограничения**: canEdit/canDelete, field-level permissions, status-based restrictions
**Валидация / бизнес-правила**: Required fields, type validation, cross-field validation, business rules (GetBusinessObjectErrors), duplicate check
**Состояния компонента**: New (create), Edit, View (read-only), Dirty (unsaved changes), Saving, Error
**Связи с другими компонентами**: DataGrid (parent list), Lookup Selectors, File Attachments, Workflow, nested DataGrids
**Требования к UX**: Tab-order для keyboard navigation, focus management, dirty state detection, confirmation on close, error highlighting
**Объем данных / частота обновления**: 1 запись, 10-50 полей, 0-5 вложенных гридов
**Признаки desktop-like поведения**: Undo/redo (CMParamListHistory), field-level permission mapping, auto-layout (AFX_DYNAMIC_LAYOUT)
**Признаки давления на стек**: rich forms, complex permissions, heavy keyboard navigation
**Приоритет**: must
**Уверенность**: high

**Историческая реализация**: CBaseCrudDialog с controlToParamMap, BusinessObject validation, undo/redo history. Каждый контрол маппится на параметр через ID.
**Web-альтернатива**: Form library с schema-driven validation, dirty tracking, field-level permissions. React Hook Form / Formik / VeeValidate + Zod/Yup.

---

**Компонент**: Inline Grid Editor
**Тип**: Editing Pattern
**Где используется**: Строки документов (MoveMaterialLines, InOrderLines, WriteOffMaterialLines, SpecificationLines, etc.)
**Основная задача пользователя**: Быстро вводить/редактировать строки документа прямо в гриде без открытия отдельной формы
**Роли**: Пользователи с правом редактирования
**Что читает из системы**: Строки документа, lookup-данные для combo-ячеек
**Что изменяет в системе**: INSERT/UPDATE/DELETE строк документа
**Команды / действия**: Click cell → enter edit, Tab → next cell, Enter → confirm, Escape → cancel, Delete row, Add row
**Запросы / чтение**: Строки по ParentID, lookup данные
**Права доступа / ограничения**: Редактирование зависит от статуса документа (черновик → можно, проведён → нельзя)
**Валидация / бизнес-правила**: Тип ячейки (число, дата, lookup), min/max, required, calculated fields (итого, НДС)
**Состояния компонента**: Display, Cell editing, Row adding, Validating
**Связи с другими компонентами**: DataGrid (host), CRUD-form (parent document), Lookup Selectors
**Требования к UX**: Keyboard-first (Tab/Enter/Escape), instant feedback, auto-calculation, cell-level error display
**Объем данных / частота обновления**: 1-500 строк, real-time при вводе
**Признаки desktop-like поведения**: Excel-like cell editing with 10+ editor types
**Признаки давления на стек**: advanced data grid, heavy keyboard navigation, desktop-like interaction density
**Приоритет**: must
**Уверенность**: high

---

### 3.3. Lookup & Selection

---

**Компонент**: Lookup Selector (ComboBox with Search)
**Тип**: Selection Control
**Где используется**: Каждая CRUD-форма (5-15 lookup-ов на форму), фильтры в списках
**Основная задача пользователя**: Быстро выбрать связанную сущность (контрагент, номенклатура, склад, сотрудник, договор, проект)
**Роли**: Все
**Что читает из системы**: Справочные данные (Contragents, Nomenclatura, Storage, Employees, etc.)
**Что изменяет в системе**: Устанавливает FK-ссылку в родительской записи
**Команды / действия**: Type to search (autocomplete), Click button → open full selector dialog, Clear selection
**Запросы / чтение**: SELECT с фильтром LIKE по введённому тексту, limited results
**Права доступа / ограничения**: Фильтрация по tenant, по проекту, по подразделению
**Валидация / бизнес-правила**: Required field check, valid reference check
**Состояния компонента**: Empty, Searching, Selected, Disabled, Error (invalid selection)
**Связи с другими компонентами**: CRUD-form (parent), Selection Dialog (expanded picker), DataGrid (inline combo cell)
**Требования к UX**: Autocomplete с debounce, display text ≠ value, clear button, button для расширенного выбора
**Объем данных / частота обновления**: 10-50000 записей в справочнике, виртуализированный dropdown
**Признаки desktop-like поведения**: MSelectorCombo — combo + кнопка поиска + clear button
**Признаки давления на стек**: rich forms (autocomplete + modal picker)
**Приоритет**: must
**Уверенность**: high

**Историческая реализация**: CMSelectorCombo (устаревший) → CContainerComboBox (новый). Combo с кнопкой "..." для расширенного поиска.
**Web-альтернатива**: Combobox/Select с async search + modal picker для сложных случаев. Стандартный web-паттерн.

---

**Компонент**: Selection Dialog (Picker Modal)
**Тип**: Modal Dialog
**Где используется**: 28+ специализированных диалогов выбора (сотрудник, контрагент, номенклатура, спецификация, договор, роль, должность, etc.)
**Основная задача пользователя**: Найти и выбрать запись из большого справочника с расширенными фильтрами
**Роли**: Все
**Что читает из системы**: Справочные таблицы с фильтрацией
**Что изменяет в системе**: Ничего (возвращает выбранный ID)
**Команды / действия**: Search, Filter, Select (single/multi), OK, Cancel
**Запросы / чтение**: Параметризованные SELECT с пагинацией
**Права доступа / ограничения**: Фильтрация доступных записей по ролям
**Валидация / бизнес-правила**: Проверка что выбранная запись ещё активна (не удалена)
**Состояния компонента**: Loading, Searching, ResultsDisplayed, Selected, Empty
**Связи с другими компонентами**: Lookup Selector (вызывающий), DataGrid (для отображения результатов)
**Требования к UX**: Быстрый поиск, DataGrid внутри модала, multi-select для batch operations, keyboard support (Enter = OK)
**Объем данных / частота обновления**: 100-50000 записей, пагинация/виртуализация
**Признаки desktop-like поведения**: Modal dialog с встроенным гридом и фильтрами
**Признаки давления на стек**: advanced data grid (внутри модала), rich forms
**Приоритет**: must
**Уверенность**: high

---

### 3.4. Document Workflow & Status

---

**Компонент**: Document Workflow (Status Transitions)
**Тип**: Workflow Engine
**Где используется**: 15+ типов документов — MoveMaterials, WriteOffMaterials, InOrders, ProductSales, Contracts, QualityControlTask, TechnicalSupervision, EquipmentOrder, TOIRService, ConstructionObject, PersonalInstruction
**Основная задача пользователя**: Перевести документ из одного статуса в другой (черновик → проведён, создан → на исполнении → выполнен)
**Роли**: Зависит от документа (автор, согласующий, исполнитель, руководитель)
**Что читает из системы**: Текущий статус документа, допустимые переходы, привилегии
**Что изменяет в системе**: UPDATE статуса + бизнес-эффекты (проведение → обновление остатков, списание → создание проводок)
**Команды / действия**: Execute (провести), Unexecute (отменить проведение), Approve, Reject, Send, Complete, Cancel
**Запросы / чтение**: Текущий статус, допустимые переходы, зависимые документы
**Права доступа / ограничения**: Привилегии на каждый переход (Execute privilege, Unexecute privilege)
**Валидация / бизнес-правила**: Проверка всех обязательных полей перед проведением, проверка зависимых документов перед отменой, бизнес-правила проведения (достаточность остатков для списания)
**Состояния компонента**: Per-document: Draft → Executed (simple) или CREATE → ONEXEC → EXEC/ELIMINATED → ACCEPTED (complex)
**Связи с другими компонентами**: CRUD-form, Context Toolbar, Notification system, Audit/History
**Требования к UX**: Кнопки workflow в toolbar, visual status indicator, confirmation dialogs с предупреждениями, отображение цепочки переходов
**Объем данных / частота обновления**: 1 документ, при каждом переходе
**Признаки desktop-like поведения**: Транзакционность (ScopedTransaction), каскадные эффекты
**Признаки давления на стек**: workflow/status engine, complex permissions
**Приоритет**: must
**Уверенность**: high

**Бизнес-требование**: Документ-ориентированная ERP. Каждый документ имеет жизненный цикл.
**Web-альтернатива**: State machine на backend (API endpoint per transition). UI показывает доступные действия. Можно использовать finite state machine library.

---

**Компонент**: Concord (Approval Routing)
**Тип**: Workflow / Approval
**Где используется**: Contract, PriceProtocol, Payment, PersonalInstruction, ProjectJobReport, Specification, OutgoingCorrespondence, IncomingCorrespondence (10+ типов документов)
**Основная задача пользователя**: Поставить документ на согласование по маршруту, согласовать/отклонить в кабинете задач
**Роли**: Автор (создаёт маршрут), Согласующий (утверждает/отклоняет), Наблюдатель
**Что читает из системы**: ConcordRoute (маршрут), ConcordStepPath (шаги), ConcordOperation (операции), текущий статус
**Что изменяет в системе**: Создание маршрута, регистрация документа на маршрут, согласование/отклонение шага, уведомления
**Команды / действия**: CreateRoute, AssignSteps, RegisterDocument, Approve, Reject, AddComment, ViewHistory
**Запросы / чтение**: Маршрут, шаги, статусы, история операций
**Права доступа / ограничения**: Только участники маршрута могут согласовать, автор не может согласовать свой документ
**Валидация / бизнес-правила**: Все шаги должны быть назначены, порядок шагов, обязательный комментарий при отклонении
**Состояния компонента**: Draft route → Registered → In approval → Step approved/rejected → All approved / Rejected
**Связи с другими компонентами**: CRUD-form (документ), Personal Task List (кабинет задач), Notification, History/Audit
**Требования к UX**: Визуализация маршрута (шаги с иконками статуса), timeline согласования, быстрые действия из кабинета задач
**Объем данных / частота обновления**: 3-10 шагов на документ, обновление при каждом действии
**Признаки desktop-like поведения**: Modal dialogs для настройки маршрута
**Признаки давления на стек**: workflow/status engine, complex permissions, real-time updates
**Приоритет**: must
**Уверенность**: high

---

**Компонент**: Personal Task List (Кабинет задач)
**Тип**: Dashboard / Task Inbox
**Где используется**: Единый входящий список задач для согласования
**Основная задача пользователя**: Видеть все назначенные задачи, быстро согласовать/отклонить, перейти к документу
**Роли**: Все (каждый видит свои задачи)
**Что читает из системы**: ConcordStepPath (назначенные шаги), типы документов, статусы
**Что изменяет в системе**: Mark as viewed/unviewed, navigate to document
**Команды / действия**: View, Mark read/unread, Open source document, Approve/Reject (quick action)
**Запросы / чтение**: Tasks for current user, grouped by status/type
**Права доступа / ограничения**: Только свои задачи
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: All tasks / Unread only / By type filters
**Связи с другими компонентами**: Concord system, Navigation (open document), Badge counters
**Требования к UX**: Быстрое сканирование (grid), group by document type, unread counter, one-click navigation to document
**Объем данных / частота обновления**: 10-200 задач, обновление каждые 30-60 сек (или push)
**Признаки desktop-like поведения**: Нет (стандартный inbox pattern)
**Признаки давления на стек**: real-time updates (push notifications)
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Task inbox / notification center. Идеально подходит для web + push notifications.

---

### 3.5. Filters & Search

---

**Компонент**: Filter Panel
**Тип**: Filter / Search
**Где используется**: Все list-формы (~54 списка) — фильтрация по дате, статусу, проекту, подразделению, контрагенту
**Основная задача пользователя**: Отфильтровать список документов по критериям
**Роли**: Все
**Что читает из системы**: Справочники для combo-фильтров (проекты, статусы, подразделения)
**Что изменяет в системе**: Ничего (фильтрация UI / запрос к API)
**Команды / действия**: Set filter → Apply → Refresh grid, Clear filters, Save filter preset
**Запросы / чтение**: Запрос к API с параметрами фильтрации
**Права доступа / ограничения**: Фильтрация по доступным проектам/подразделениям
**Валидация / бизнес-правила**: Date range validation (from ≤ to), valid enum values
**Состояния компонента**: Default, Custom (filters applied), Saved preset loaded
**Связи с другими компонентами**: DataGrid (data consumer), Lookup Selectors (filter inputs)
**Требования к UX**: Horizontal or sidebar layout, date range pickers, combo selectors, instant apply, clear all, collapsible, remember last filter
**Объем данных / частота обновления**: 5-15 фильтров на форму, apply triggers grid reload
**Признаки desktop-like поведения**: CComboBoxEx, CDateTimeCtrl, CButton (checkbox) в ribbon или на форме
**Признаки давления на стек**: basic CRUD UI
**Приоритет**: must
**Уверенность**: high

---

**Компонент**: Global Search
**Тип**: Search
**Где используется**: Ribbon search bar (IDE_SEARCH, 3708), поисковая строка на каждой list-форме
**Основная задача пользователя**: Быстро найти документ/запись по номеру, названию или фрагменту
**Роли**: Все
**Что читает из системы**: Текущий список (фильтрация по введённой строке)
**Что изменяет в системе**: Ничего
**Команды / действия**: Type text → filter, Enter → apply, Clear → reset
**Запросы / чтение**: LIKE filter на текущий грид или full-text search
**Права доступа / ограничения**: Scope ограничен текущей формой
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Empty, Typing, Results filtered, No results
**Связи с другими компонентами**: DataGrid (filtered), Context Toolbar (search edit)
**Требования к UX**: Instant filter (debounce 300ms), clear button, numeric search mode, highlight matches
**Объем данных / частота обновления**: Keystroke-triggered
**Признаки desktop-like поведения**: RICHEDIT50W в ribbon (нестандартный контрол)
**Признаки давления на стек**: basic CRUD UI
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Search input с debounce. Стандартный паттерн. Можно добавить omni-search (глобальный поиск по всем модулям).

---

### 3.6. Reports & Export

---

**Компонент**: Report Generator (Print/Export)
**Тип**: Report / Export
**Где используется**: Все документальные модули — 58 отчётов (24 Excel + 34 HTML)
**Основная задача пользователя**: Сформировать печатную форму документа (М-15, ТОРГ-12, КС-2, акт, накладная) или аналитический отчёт
**Роли**: Все (зависит от модуля)
**Что читает из системы**: Данные документа + шаблон отчёта
**Что изменяет в системе**: Ничего
**Команды / действия**: Print (→ browser print), Export to Excel, Export to HTML, Preview
**Запросы / чтение**: Данные документа / отчёта
**Права доступа / ограничения**: Привилегия на печать / экспорт
**Валидация / бизнес-правила**: Документ должен быть сохранён перед печатью
**Состояния компонента**: Idle, Generating, Preview, Printing, Error
**Связи с другими компонентами**: CRUD-form (source data), Context Toolbar (Print button)
**Требования к UX**: Preview перед печатью, выбор формата (PDF/Excel/HTML), progress indicator для больших отчётов
**Объем данных / частота обновления**: On-demand, может занять 1-30 сек для больших отчётов
**Признаки desktop-like поведения**: COM automation для Excel, HTML generation в C++
**Признаки давления на стек**: print/export, file/document handling, background async operations
**Приоритет**: must
**Уверенность**: high

**Историческая реализация**: MExcelReportBase (COM → Excel), MHTMLReportBase (HTML generation). Формы КС-2, ТОРГ-12, М-15 — государственные формы с фиксированным layout.
**Web-альтернатива**: Server-side PDF/Excel generation (weasyprint, openpyxl, reportlab). Client preview в iframe. Это серверная задача, не frontend.

---

**Компонент**: Excel Import
**Тип**: Import Wizard
**Где используется**: Спецификации, Ценовые протоколы, DocApprove, Материалы договоров
**Основная задача пользователя**: Загрузить данные из Excel файла в систему с маппингом колонок
**Роли**: Зависит от модуля
**Что читает из системы**: Целевую структуру таблицы (для маппинга)
**Что изменяет в системе**: INSERT/UPDATE записей из файла
**Команды / действия**: Upload file, Map columns, Validate, Import, View errors
**Запросы / чтение**: Структура целевой таблицы, validation rules
**Права доступа / ограничения**: Привилегия на импорт
**Валидация / бизнес-правила**: Обязательные колонки, тип данных, дубликаты, referential integrity
**Состояния компонента**: Upload → Preview → Column mapping → Validation → Import → Results
**Связи с другими компонентами**: DataGrid (preview imported data), Error display
**Требования к UX**: Drag-drop file upload, preview table, column mapping UI, validation summary, partial import (skip errors)
**Объем данных / частота обновления**: 10-10000 строк за один импорт
**Признаки desktop-like поведения**: Multi-step wizard (PropertySheet)
**Признаки давления на стек**: file/document handling, rich forms
**Приоритет**: should
**Уверенность**: high

---

### 3.7. File Attachments

---

**Компонент**: File Attachment Manager
**Тип**: File Handling
**Где используется**: DocApprove (файлы документации), Correspondence (вложения писем), Contracts, QualityControl, ProjectJobReport, EquipmentOrder
**Основная задача пользователя**: Прикрепить файлы к документу, скачать, просмотреть, удалить
**Роли**: Все с правом редактирования документа
**Что читает из системы**: FileStream / FileDocumentLink, файловое хранилище
**Что изменяет в системе**: Upload file → FileStream, Link file to document, Delete link
**Команды / действия**: Upload, Download, Preview, Delete, Open in external app
**Запросы / чтение**: Список файлов по документу
**Права доступа / ограничения**: canEdit на документ, file-level locks (FileLock)
**Валидация / бизнес-правила**: Max file size, allowed extensions (configurable), virus scan (potential)
**Состояния компонента**: Empty, FileList, Uploading, Downloading, Error
**Связи с другими компонентами**: CRUD-form (parent document), File Archive
**Требования к UX**: Drag-drop upload, file list with icons, preview for common types (PDF, images), progress bar
**Объем данных / частота обновления**: 0-50 файлов на документ, on-demand
**Признаки desktop-like поведения**: CFileOperation, file archive browser
**Признаки давления на стек**: file/document handling
**Приоритет**: must
**Уверенность**: high

---

### 3.8. Notifications & Feedback

---

**Компонент**: Toast Notification System
**Тип**: Notification
**Где используется**: Все модули — уведомления о согласовании, ошибки, успешные операции
**Основная задача пользователя**: Видеть результат операции, уведомления о новых задачах
**Роли**: Все
**Что читает из системы**: Notification events (Concord, background tasks)
**Что изменяет в системе**: Mark notification as read
**Команды / действия**: Dismiss, Click to navigate
**Запросы / чтение**: Notification queue
**Права доступа / ограничения**: Только свои уведомления
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Hidden, Showing (fade in), Visible (auto-dismiss 5s), Dismissing (fade out)
**Связи с другими компонентами**: Background jobs, Concord system, Task List
**Требования к UX**: Non-blocking, auto-dismiss, clickable (navigate to source), stacking for multiple
**Объем данных / частота обновления**: Event-driven, 0-5 в минуту
**Признаки desktop-like поведения**: CToastPopupWnd с fade-анимацией
**Признаки давления на стек**: real-time updates
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Toast library (стандартный паттерн) + WebSocket/SSE для push notifications.

---

**Компонент**: Error / Validation Display
**Тип**: Feedback
**Где используется**: Все CRUD-формы — отображение ошибок валидации и бизнес-правил
**Основная задача пользователя**: Понять что не так и исправить
**Роли**: Все
**Что читает из системы**: GetBusinessObjectErrors(), field-level validation results
**Что изменяет в системе**: Ничего
**Команды / действия**: Click error → focus on field
**Запросы / чтение**: Нет
**Права доступа / ограничения**: Нет
**Валидация / бизнес-правила**: Displays results of validation
**Состояния компонента**: No errors, Errors present (list), Single error (inline)
**Связи с другими компонентами**: CRUD-form, DataGrid (cell-level errors)
**Требования к UX**: Error summary (top of form or toast), field highlighting (red border), click-to-focus, clear on fix
**Объем данных / частота обновления**: On save attempt
**Признаки desktop-like поведения**: CErrorShowDlg — modal error list
**Признаки давления на стек**: basic CRUD UI
**Приоритет**: must
**Уверенность**: high

**Web-альтернатива**: Inline validation + error summary. Стандартный web-паттерн, лучше чем modal error list.

---

**Компонент**: Confirmation Dialog
**Тип**: Modal / Feedback
**Где используется**: Все деструктивные операции — удаление, отмена проведения, закрытие без сохранения
**Основная задача пользователя**: Подтвердить или отменить опасное действие
**Роли**: Все
**Что читает из системы**: Контекст операции
**Что изменяет в системе**: Ничего (подтверждение для вызывающего)
**Команды / действия**: Confirm, Cancel
**Запросы / чтение**: Нет
**Права доступа / ограничения**: Нет
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Open, Confirmed, Cancelled
**Связи с другими компонентами**: Любой компонент с деструктивными операциями
**Требования к UX**: Чёткое описание последствий, focus на Cancel (safe default), keyboard (Escape = Cancel)
**Объем данных / частота обновления**: On-demand
**Признаки desktop-like поведения**: AfxMessageBox
**Признаки давления на стек**: basic CRUD UI
**Приоритет**: must
**Уверенность**: high

---

### 3.9. Specialized Components

---

**Компонент**: Gantt Chart
**Тип**: Visualization
**Где используется**: Project Management, Production Plan, TOIR Graph
**Основная задача пользователя**: Визуализировать расписание работ/задач на временной шкале
**Роли**: PM, Плановик, Гл. механик
**Что читает из системы**: GanttGraph, GanttGraphData, ProjectJobs, ProductionTasks с датами
**Что изменяет в системе**: Drag bars → update dates (потенциально), create/edit tasks
**Команды / действия**: Zoom in/out, Scroll timeline, Click bar → details, Create task, Edit dates
**Запросы / чтение**: Задачи с start/end dates, dependencies
**Права доступа / ограничения**: canEdit на проект/план
**Валидация / бизнес-правила**: Date constraints, dependency validation
**Состояния компонента**: Loading, Displaying, Scrolling, Editing
**Связи с другими компонентами**: DataGrid (task list), CRUD-form (task details), Export
**Требования к UX**: Horizontal scroll, zoom levels (day/week/month), task bars with colors, dependencies lines, milestones, today marker
**Объем данных / частота обновления**: 50-500 задач, on-demand refresh
**Признаки desktop-like поведения**: Custom GDI rendering, interactive bars
**Признаки давления на стек**: advanced data grid (gantt = special grid), desktop-like interaction density
**Приоритет**: should
**Уверенность**: high

**Web-альтернатива**: Gantt library (DHTMLX Gantt, Bryntum, Frappe Gantt). Нишевый компонент, нужна ready-made library.

---

**Компонент**: Production Line Monitor
**Тип**: Real-time Dashboard
**Где используется**: Мониторинг производственных линий
**Основная задача пользователя**: Видеть текущее состояние линии в реальном времени, ошибки, производительность
**Роли**: Оператор, Мастер, Руководство
**Что читает из системы**: Modbus данные с оборудования, error logs, production counters
**Что изменяет в системе**: Manual mode controls, error acknowledgement
**Команды / действия**: View status, Acknowledge error, Switch mode, Export log
**Запросы / чтение**: Real-time polling (Modbus TCP/RTU), historical data
**Права доступа / ограничения**: View-only для операторов, control для мастеров
**Валидация / бизнес-правила**: Safety interlocks, alarm thresholds
**Состояния компонента**: Online, Offline, Error, Manual mode, Auto mode
**Связи с другими компонентами**: Error Group management, Production Task
**Требования к UX**: Auto-refresh (1-5 sec), color-coded indicators, alarm sounds (потенциально), large font for shop floor displays
**Объем данных / частота обновления**: Real-time (1-5 sec polling), 10-50 data points per line
**Признаки desktop-like поведения**: Modbus direct connection, custom GDI rendering
**Признаки давления на стек**: real-time updates, desktop-like interaction density
**Приоритет**: could (если производственный модуль в scope)
**Уверенность**: medium

**Историческая реализация**: Direct Modbus TCP/RTU через libmodbus. В web-версии Modbus останется на backend (gateway), frontend получает данные через WebSocket/SSE.

---

**Компонент**: Document Link Tree
**Тип**: Visualization / Navigation
**Где используется**: DocLinkTree — граф связей между документами
**Основная задача пользователя**: Увидеть все связанные документы (потребность → заказ → приход → перемещение → списание) и перейти к нужному
**Роли**: Все
**Что читает из системы**: DocumentLinkTree (TVF), цепочка FK-связей
**Что изменяет в системе**: Ничего (навигация)
**Команды / действия**: Expand node, Click → navigate to document
**Запросы / чтение**: Рекурсивный запрос связей
**Права доступа / ограничения**: Видимость ограничена правами на каждый тип документа
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Loading, Displayed, Node selected
**Связи с другими компонентами**: Shell (navigate to document tab), все документные формы
**Требования к UX**: Иерархическое дерево с иконками типов документов, клик → открыть документ
**Объем данных / частота обновления**: 5-50 связанных документов
**Признаки desktop-like поведения**: TreeView с навигацией
**Признаки давления на стек**: basic CRUD UI
**Приоритет**: should
**Уверенность**: high

---

**Компонент**: Barcode Scanner Integration
**Тип**: Device Integration
**Где используется**: EquipmentOrderList (поиск по штрих-коду), складские операции
**Основная задача пользователя**: Отсканировать штрих-код для быстрого поиска/идентификации
**Роли**: Кладовщик, Оператор
**Что читает из системы**: Lookup по коду
**Что изменяет в системе**: Ничего (поиск)
**Команды / действия**: Scan → auto-search
**Запросы / чтение**: SELECT WHERE Barcode = scanned_value
**Права доступа / ограничения**: Нет
**Валидация / бизнес-правила**: Valid barcode format
**Состояния компонента**: Ready, Scanning, Found, Not found
**Связи с другими компонентами**: DataGrid (filter by barcode), CRUD-form (auto-fill)
**Требования к UX**: Auto-focus на поле ввода, USB/keyboard-emulation scanners just work, camera scan для mobile
**Объем данных / частота обновления**: On-demand
**Признаки desktop-like поведения**: COM port direct access (legacy)
**Признаки давления на стек**: basic CRUD UI (keyboard-emulation scanners work natively in web)
**Приоритет**: could
**Уверенность**: medium

**Web-альтернатива**: USB scanners работают как keyboard input — поддерживается нативно. Camera scan через WebRTC API для мобильных.

---

**Компонент**: Multi-Tab Document Form
**Тип**: Rich Form Layout
**Где используется**: DocApprove (5 вкладок: каталог, документы, шаги, замечания, поиск), Contract (реквизиты, спецификация, финансы), PersonalInstruction (вкладки)
**Основная задача пользователя**: Работать с разными аспектами сложного документа
**Роли**: Зависит от модуля
**Что читает из системы**: Множество связанных данных одного документа
**Что изменяет в системе**: Данные на каждой вкладке
**Команды / действия**: Switch tab, Edit on each tab, Save all
**Запросы / чтение**: Данные для каждой вкладки (lazy load)
**Права доступа / ограничения**: Tab-level visibility по ролям
**Валидация / бизнес-правила**: Cross-tab validation (все вкладки валидны перед сохранением)
**Состояния компонента**: Tab active, Tab dirty, Tab error, All clean
**Связи с другими компонентами**: CRUD-form (each tab), DataGrid, TreeView
**Требования к UX**: Dirty indicator per tab, error indicator per tab, lazy loading, maintain state between tab switches
**Объем данных / частота обновления**: 3-7 вкладок, on-demand loading
**Признаки desktop-like поведения**: CPropertySheet / Tab Control
**Признаки давления на стек**: rich forms, desktop-like interaction density
**Приоритет**: must
**Уверенность**: high

---

**Компонент**: Batch / Mass Operations
**Тип**: Action Pattern
**Где используется**: Production tasks (complete multiple), Specifications (mass replace), Auxiliary tasks (mass close), DocApprove (update status batch)
**Основная задача пользователя**: Выполнить одно действие над несколькими записями сразу
**Роли**: Зависит от модуля
**Что читает из системы**: Выбранные записи в гриде
**Что изменяет в системе**: UPDATE/DELETE нескольких записей
**Команды / действия**: Select multiple (checkbox), Choose action, Confirm, Execute
**Запросы / чтение**: Selected IDs
**Права доступа / ограничения**: Привилегия на операцию + проверка каждой записи
**Валидация / бизнес-правила**: Все записи должны быть в допустимом статусе, partial success handling
**Состояния компонента**: Selection, Confirmation, Processing (progress), Results (success/partial/fail)
**Связи с другими компонентами**: DataGrid (multi-select), Progress indicator, Error summary
**Требования к UX**: Checkbox column in grid, "Select all" option, progress bar, error report after completion
**Объем данных / частота обновления**: 2-100 записей за раз
**Признаки desktop-like поведения**: CAuxTaskMassCloseDlg — специальный диалог
**Признаки давления на стек**: background async operations, advanced data grid
**Приоритет**: should
**Уверенность**: high

---

**Компонент**: Audit / History Trail
**Тип**: Information Display
**Где используется**: Concord (история согласований), Documents (история изменений), потенциально все сущности
**Основная задача пользователя**: Увидеть кто, когда и что изменил
**Роли**: Руководство, аудиторы
**Что читает из системы**: Audit trail таблица, ConcordOperation history
**Что изменяет в системе**: Ничего
**Команды / действия**: View history, Filter by date/user, Export
**Запросы / чтение**: Audit log SELECT с фильтрами
**Права доступа / ограничения**: Привилегия на просмотр аудита
**Валидация / бизнес-правила**: Нет
**Состояния компонента**: Loading, Displayed, Filtered
**Связи с другими компонентами**: CRUD-form (source document), DataGrid (history display)
**Требования к UX**: Timeline view или table, diff view для изменений (что было → что стало)
**Объем данных / частота обновления**: 10-1000 записей на документ, read-only
**Признаки desktop-like поведения**: CApprovalHistoryDlg — modal dialog с историей
**Признаки давления на стек**: basic CRUD UI, large dataset / virtualization
**Приоритет**: should
**Уверенность**: high

---

**Компонент**: "Create Based On" (Create-from)
**Тип**: Action Pattern
**Где используется**: IncomingCorrespondence → PersonalInstruction, Need → SupplierOrder, Need → MoveMaterial, SupplierOrder → InOrder, ProductionTask → WriteOff
**Основная задача пользователя**: Создать новый документ на основании существующего с предзаполнением полей
**Роли**: Зависит от модуля
**Что читает из системы**: Исходный документ (все поля для копирования)
**Что изменяет в системе**: INSERT нового документа со скопированными полями
**Команды / действия**: "Создать на основании" → выбор типа → новая форма с prefill
**Запросы / чтение**: Исходный документ, маппинг полей
**Права доступа / ограничения**: Привилегия на создание целевого документа
**Валидация / бизнес-правила**: Маппинг полей, пересчёт значений, обязательные поля целевого документа
**Состояния компонента**: Source selected → Type chosen → New form opened with prefill
**Связи с другими компонентами**: CRUD-form (source and target), Document Link Tree
**Требования к UX**: Menu/dropdown для выбора типа целевого документа, clearly show which fields copied, allow editing before save
**Объем данных / частота обновления**: On-demand, 1 document
**Признаки desktop-like поведения**: CreateBasedOn button в ribbon
**Признаки давления на стек**: rich forms
**Приоритет**: should
**Уверенность**: high

---

**Компонент**: Permission Matrix
**Тип**: Admin Tool
**Где используется**: Roles management, Department permissions, Project permissions
**Основная задача пользователя**: Назначить права доступа по ролям / модулям
**Роли**: Администратор
**Что читает из системы**: Roles, Functions, Permissions, Users
**Что изменяет в системе**: GRANT/REVOKE permission entries
**Команды / действия**: Toggle permission (checkbox), Save matrix
**Запросы / чтение**: Role-Permission cross-join
**Права доступа / ограничения**: Только администраторы
**Валидация / бизнес-правила**: Cascading permissions, conflicting rules detection
**Состояния компонента**: Loading, Editing, Saving, Saved
**Связи с другими компонентами**: User management, Role management
**Требования к UX**: Cross-table (roles × permissions), bulk toggle (entire row/column), search/filter permissions
**Объем данных / частота обновления**: 20-200 roles × 100-500 permissions, infrequent changes
**Признаки desktop-like поведения**: Grid of checkboxes
**Признаки давления на стек**: advanced data grid (checkbox matrix), complex permissions
**Приоритет**: must
**Уверенность**: high

---

## 4. Cross-Cutting Capabilities

### 4.1. Единый Shell (MUST)
- SPA с tab-based navigation
- Context-sensitive toolbar per active tab
- Sidebar navigation с role-based filtering и badge counters
- Breadcrumb / route display
- User menu (profile, settings, logout, tenant switch)

### 4.2. Authentication & Authorization (MUST)
- JWT/session auth с refresh tokens
- Multi-tenant isolation (tenant context в каждом запросе)
- Role-based access control на 3 уровнях:
  - **Menu/Navigation**: скрытие недоступных пунктов
  - **Form/Button**: disable/hide кнопок и полей
  - **Data**: фильтрация записей по tenant/project/department
- Session expiry handling (redirect to login, preserve unsaved work)

### 4.3. Единые Lookup Selectors (MUST)
- Переиспользуемый компонент для выбора из справочника
- Autocomplete с server-side search
- Расширенный picker (modal с grid и фильтрами)
- Кэширование часто используемых справочников
- Конфигурируемый: display field, value field, search fields, filters

### 4.4. Единый Data Grid (MUST)
- Enterprise-grade grid с:
  - Column sorting, resizing, reordering
  - Column pinning (freeze)
  - Virtual scrolling (10000+ rows)
  - Multi-select (checkbox column)
  - Inline editing (text, number, date, combo, checkbox)
  - Cell/Row coloring
  - Context menu
  - Export to Excel/CSV
  - Column configuration persistence (user preferences)
  - Grouped rows (expandable groups)

### 4.5. Единая Form Engine (MUST)
- Schema-driven forms с:
  - Field types: text, number, date, datetime, combo, checkbox, textarea, file
  - Validation: required, min/max, regex, custom, cross-field, async (server-side)
  - Dirty tracking
  - Field-level permissions (read-only per role)
  - Auto-layout (responsive)
  - Tab navigation (keyboard)

### 4.6. Document Workflow Engine (MUST)
- State machine driven status transitions
- Visual status indicator (badge/chip with color)
- Available actions based on current status + user role
- Transition side-effects (server-side)
- Audit trail per transition

### 4.7. Approval Routing (Concord) (MUST)
- Route builder UI (steps, assignees, order)
- Approval/rejection actions
- Comment on approval
- Task inbox (Personal Task List)
- Notification on status change (email + in-app)

### 4.8. Notifications (MUST)
- In-app toast notifications (non-blocking)
- Push notifications (WebSocket/SSE)
- Notification center (history)
- Email notifications (server-side)
- Badge counters on navigation items

### 4.9. File Attachments (MUST)
- Upload (drag-drop + file picker)
- Download
- Preview (PDF, images inline)
- File list per document
- Progress indication
- Max size / type restrictions

### 4.10. Print / Export (MUST)
- Server-generated PDF (for regulated forms: КС-2, ТОРГ-12, М-15)
- Excel export (grid data + formatted reports)
- Browser print (simple reports)
- Print preview

### 4.11. Audit / History (SHOULD)
- Change history per entity
- Who changed what, when
- Diff view (old → new values)
- Approval history (Concord steps)

### 4.12. Localization (SHOULD)
- Russian (primary) + English (secondary)
- Date/number formatting per locale
- Translatable strings
- RTL не требуется

### 4.13. Error Handling (MUST)
- Validation errors (field-level + summary)
- Business rule violations (server-side → displayed on form)
- Network errors (retry, offline indicator)
- Session expiry (redirect to login)
- Unhandled errors (error boundary + reporting)

### 4.14. User Preferences (SHOULD)
- Grid column configuration per user
- Filter presets
- Theme (light/dark — optional)
- Default filters / views
- Language selection

### 4.15. Keyboard Navigation (SHOULD)
- Tab order in forms
- Arrow keys in grids
- Hotkeys for common actions (Ctrl+S, Ctrl+N, Escape)
- Focus management (auto-focus on open)
- Keyboard-accessible modals

---

## 5. Stack-Pressure Summary

### Level 1: Базовый web UI (покрывается любым фреймворком)

| Capability | Описание |
|------------|----------|
| SPA routing | Tab-based navigation, route guards |
| Basic forms | Text, number, date inputs с validation |
| Auth / JWT | Login, refresh tokens, session management |
| Toast notifications | Non-blocking messages |
| Confirmation dialogs | Simple modals |
| Status badges | Color-coded status indicators |
| Breadcrumbs | Navigation context |
| Responsive layout | Basic responsive grid layout |
| Error boundaries | Global error handling |

### Level 2: Требует зрелой component library

| Capability | Описание | Давление |
|------------|----------|----------|
| Rich forms с 10+ field types | Combo, date, file, textarea, checkbox, nested sections | rich forms |
| Lookup selectors с async search | Autocomplete + modal picker | rich forms |
| Filter panels | Date range, combo, checkbox, saved presets | rich forms |
| Multi-tab forms | Tabs с dirty/error indicators | rich forms |
| File upload/download | Drag-drop, progress, preview | file/document handling |
| Data table с sorting/filtering | Basic table with headers, pagination | basic CRUD UI |
| Role-based visibility | Conditional rendering by permissions | complex permissions |
| Form dirty tracking | Unsaved changes detection | rich forms |

### Level 3: Требует enterprise-grade grid/form/workflow поддержки

| Capability | Описание | Давление |
|------------|----------|----------|
| **Advanced DataGrid** | Inline editing (10+ cell types), virtual scroll, drag-drop, grouping, cell coloring, context menu, clipboard, column config | **advanced data grid** |
| **Tree-Grid hybrid** | Hierarchical tree + grid с master-detail | advanced data grid |
| **Workflow engine UI** | Status transitions, approval routing, visual state machine | workflow/status engine |
| **Batch operations** | Multi-select + mass action + progress + partial results | background async operations |
| **Complex permissions** | Field-level, button-level, data-level permissions | complex permissions |
| **Excel import wizard** | Multi-step: upload → preview → map → validate → import | rich forms, file handling |
| **Print/PDF generation** | Server-side PDF/Excel for regulated forms | print/export |
| **Keyboard-first editing** | Tab/Enter/Escape flow in grids, hotkeys | heavy keyboard navigation |

### Level 4: Desktop-like UX — может радикально повлиять на архитектуру

| Capability | Описание | Давление | Решение |
|------------|----------|----------|---------|
| **Enterprise grid как основной UI** | 80% времени пользователь в гриде с inline editing, 10+ cell types, drag-drop между гридами | **КРИТИЧЕСКОЕ** | Нужна enterprise grid library (AG Grid / Handsontable / аналог). Самописный grid = провал. |
| **MDI-подобная работа** | 5-15 одновременных вкладок, каждая с состоянием, context toolbar | **Высокое** | SPA с keep-alive tabs и route-based state management. Может потребовать custom tab manager. |
| **Gantt chart** | Интерактивный Gantt с drag-resize bars, dependencies, zoom | **Среднее** | Gantt library (DHTMLX, Bryntum). Нишевый компонент, не влияет на выбор основного стека. |
| **Real-time monitoring** | Modbus/IoT data → live dashboard с 1-5 sec refresh | **Среднее** | WebSocket + отдельный микросервис-gateway. Frontend: SSE/WebSocket consumer + dashboard widgets. |
| **Interaction density** | 5-15 гридов + 10 комбобоксов + деревья + вкладки + ribbon на одном экране (DocApprove) | **Высокое** | Нужен фреймворк, способный рендерить 100+ контролов без деградации. Virtual rendering обязателен. |
| **Production line ARM** | Автоматизированное рабочее место оператора линии (Modbus, ручной ввод, мониторинг) | **Низкое** | Отдельное мини-приложение (PWA), не влияет на основной стек. |

---

## 6. Open Questions

### Архитектурные

1. **Scope первой итерации**: Какие модули входят в MVP? (Склад + Производство? Или все 36 модулей?)
2. **Offline requirements**: Нужна ли offline-работа для складских/производственных сценариев (мобильные ТСД, цеховые терминалы)?
3. **Mobile requirements**: Какие сценарии нужны на мобильных устройствах? (Согласование? Склад со сканером? Мониторинг?)
4. **Real-time scope**: Производственный мониторинг (Modbus) входит в scope? Или это отдельная система?

### UI/UX

5. **Grid complexity**: Действительно ли нужен inline editing в каждом гриде? Или можно упростить до "list → open card" для части модулей?
6. **Keyboard-first**: Насколько критична keyboard-first навигация? Текущие пользователи — keyboard-heavy (бухгалтеры, кладовщики)?
7. **Concurrent editing**: Нужна ли поддержка одновременного редактирования одного документа (optimistic locking? last-write-wins? real-time co-editing)?
8. **Saved views / presets**: Нужны ли пользовательские сохранённые фильтры/представления для гридов?

### Print / Export

9. **Regulated forms**: Какие формы регулируются законодательством (КС-2, ТОРГ-12, М-15) и требуют пиксельно-точного PDF?
10. **Export scope**: Нужен ли экспорт из каждого грида или только из отчётов?

### Integration

11. **Email**: Нужна ли отправка email из UI (корреспонденция) или только серверные уведомления?
12. **External systems**: Какие интеграции нужны? (1С? банк-клиент? госуслуги? электронный документооборот?)
13. **Barcode/scanner**: Какие сценарии с штрих-кодами реально используются?

### Performance

14. **Data volume**: Какой максимальный объём данных в одном гриде? (100? 1000? 100000 строк?)
15. **Concurrent users**: Сколько одновременных пользователей? (50? 500? 5000?)
16. **Response time SLA**: Какое допустимое время отклика? (< 1 сек для CRUD? < 3 сек для отчётов?)

### Team

17. **Frontend team**: Размер и опыт фронтенд-команды? (Это влияет на выбор между "всё с нуля" и "enterprise component library".)
18. **Design system**: Будет ли единый дизайн-система? Или модули могут выглядеть по-разному?

---

## Appendix A: Итоговая таблица давления на стек

| Категория давления | Кол-во компонентов | Критичность | Примеры |
|---|---|---|---|
| basic CRUD UI | 8 | Low | Navigation, Search, Filters, Confirmations, Status Bar |
| rich forms | 6 | High | CRUD-form, Multi-tab, Lookup, Create-from, Import wizard |
| advanced data grid | 4 | **Critical** | DataGrid, TreeView, Inline editor, Permission matrix |
| workflow/status engine | 2 | High | Document workflow, Concord approval |
| heavy keyboard navigation | 3 | Medium | Grid editing, Form tab-order, Hotkeys |
| large dataset / virtualization | 2 | High | DataGrid (10K+ rows), History/Audit |
| background async operations | 2 | Medium | Batch operations, Report generation |
| real-time updates | 3 | Medium | Notifications, Task list, Monitoring |
| file/document handling | 3 | Medium | Attachments, Import, Export |
| print/export | 1 | High | Server-side PDF/Excel for regulated forms |
| complex permissions | 3 | High | Field-level, Button-level, Data-level ACL |
| desktop-like interaction density | 5 | **Critical** | Multi-grid forms, MDI tabs, Gantt, Inline editing |

### Ключевой вывод

**Enterprise Data Grid — главный фактор выбора стека**. 80% рабочего времени пользователя ERP проходит в гриде. Без enterprise-grade grid library (inline editing, virtualization, 10+ cell types, drag-drop, grouping, keyboard navigation) web ERP не сможет заменить desktop. Это единственный компонент, который нельзя "дописать позже" — он определяет архитектуру с первого дня.

**Второй фактор — rich forms engine**. 214 диалогов с маппингом контролов, cross-field validation, dirty tracking, undo/redo, field-level permissions. Schema-driven form engine сэкономит x10 времени.

**Третий фактор — workflow/permissions**. Сквозная система согласования + field/button/data-level permissions требуют глубокой интеграции в каждый компонент. Это не библиотека, это архитектурное решение.
