import type {
  ScreenDescriptor,
  ScreenId,
  DataPayload,
  BehaviorState,
  SectionDef,
  RecordDef,
  CollectionDef,
  FieldDef,
} from "@/protocol/types";

export interface RenderCallbacks {
  onNavigate?: (screenId: ScreenId, row: Record<string, unknown>) => void;
}

let _callbacks: RenderCallbacks = {};

/**
 * Renders a ScreenDescriptor + Data + Behavior into DOM elements.
 * This is the core of the UI engine — metadata → DOM.
 */
export function renderScreen(
  screen: ScreenDescriptor,
  data: DataPayload,
  behavior: BehaviorState,
  container: HTMLElement,
  callbacks?: RenderCallbacks,
): void {
  _callbacks = callbacks ?? {};
  container.innerHTML = "";

  // Card screens get a status header
  if (screen.kind === "card") {
    container.appendChild(renderCardHeader(screen, data));
  }

  for (const section of screen.sections) {
    container.appendChild(renderSection(section, data, behavior));
  }
}

function renderCardHeader(screen: ScreenDescriptor, data: DataPayload): HTMLElement {
  const header = document.createElement("div");
  header.className = "px-4 py-3 border-b border-slate-200 bg-slate-50 flex items-center gap-3";

  const title = document.createElement("h2");
  title.className = "text-lg font-semibold text-slate-800";
  const docNumber = data.record?.["doc_number"] ?? data.record?.["name"] ?? "";
  title.textContent = `${screen.title}${docNumber ? " № " + docNumber : ""}`;
  header.appendChild(title);

  const status = data.record?.["status"];
  if (status) {
    const badge = document.createElement("span");
    const statusMap: Record<string, { label: string; cls: string }> = {
      draft:    { label: "Черновик",  cls: "bg-amber-100 text-amber-800 border-amber-200" },
      posted:   { label: "Проведён",  cls: "bg-emerald-100 text-emerald-800 border-emerald-200" },
      active:   { label: "Активен",   cls: "bg-emerald-100 text-emerald-800 border-emerald-200" },
      inactive: { label: "Неактивен", cls: "bg-slate-100 text-slate-500 border-slate-200" },
    };
    const s = statusMap[String(status)] ?? { label: String(status), cls: "bg-slate-100 text-slate-600 border-slate-200" };
    badge.className = `px-2.5 py-0.5 text-xs font-semibold rounded-full transition-colors border ${s.cls}`;
    badge.textContent = s.label;
    header.appendChild(badge);
  }

  return header;
}

function renderSection(
  section: SectionDef,
  data: DataPayload,
  behavior: BehaviorState,
): HTMLElement {
  switch (section.type) {
    case "record":
      return renderRecord(section.def, data, behavior);
    case "collection":
      return renderCollection(section.def, data, behavior);
    case "tabs":
      return renderTabs(section.def, data, behavior);
  }
}

// ─── Record (form) ─────────────────────────────────────────────

function renderRecord(
  def: RecordDef,
  data: DataPayload,
  behavior: BehaviorState,
): HTMLElement {
  const wrapper = document.createElement("div");
  wrapper.className = "p-4 border-b border-slate-200";

  if (def.title) {
    const title = document.createElement("h3");
    title.className = "text-sm font-semibold text-slate-500 uppercase tracking-wide mb-3";
    title.textContent = def.title;
    wrapper.appendChild(title);
  }

  const grid = document.createElement("div");
  grid.className = `grid gap-x-6 gap-y-3 grid-cols-${def.columns}`;
  grid.style.gridTemplateColumns = `repeat(${def.columns}, minmax(0, 1fr))`;

  for (const field of def.fields) {
    const fieldState = behavior.fields[field.id];
    if (fieldState && !fieldState.visible) continue;
    grid.appendChild(renderField(field, data.record ?? {}, fieldState));
  }

  wrapper.appendChild(grid);
  return wrapper;
}

function renderField(
  def: FieldDef,
  record: Record<string, unknown>,
  state?: { editable: boolean; required: boolean },
): HTMLElement {
  const group = document.createElement("div");

  const label = document.createElement("label");
  label.className = "block text-xs font-medium text-slate-500 mb-1";
  label.textContent = def.label;
  if (state?.required) {
    label.innerHTML += '<span class="text-red-400 ml-0.5">*</span>';
  }
  group.appendChild(label);

  const value = record[def.id] ?? "";
  const editable = state?.editable ?? true;

  if (def.field_type === "textarea") {
    const textarea = document.createElement("textarea");
    textarea.className = fieldInputClass(editable);
    textarea.rows = 2;
    textarea.value = String(value);
    textarea.disabled = !editable;
    group.appendChild(textarea);
  } else if (def.field_type === "bool") {
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = Boolean(value);
    cb.disabled = !editable;
    cb.className = "rounded border-slate-300 text-accent focus:ring-accent";
    group.appendChild(cb);
  } else {
    const input = document.createElement("input");
    input.type = def.field_type === "number" ? "number" : def.field_type === "date" ? "date" : "text";
    input.className = fieldInputClass(editable);
    input.value = String(value);
    input.disabled = !editable;
    if (def.field_type === "lookup") {
      input.placeholder = `Выберите ${def.label.toLowerCase()}...`;
    }
    group.appendChild(input);
  }

  return group;
}

function fieldInputClass(editable: boolean): string {
  return [
    "w-full px-2.5 py-1.5 text-sm border rounded transition-colors",
    editable
      ? "border-slate-300 bg-white focus:border-accent focus:ring-1 focus:ring-accent/30 outline-none"
      : "border-transparent bg-slate-50 text-slate-600 cursor-default",
  ].join(" ");
}

// ─── Collection (grid/table) ───────────────────────────────────

function renderCollection(
  def: CollectionDef,
  data: DataPayload,
  _behavior: BehaviorState,
): HTMLElement {
  const wrapper = document.createElement("div");
  wrapper.className = "flex-1 flex flex-col min-h-0 p-4";

  if (def.title) {
    const title = document.createElement("h3");
    title.className = "text-sm font-semibold text-slate-500 uppercase tracking-wide mb-2";
    title.textContent = def.title;
    wrapper.appendChild(title);
  }

  const tableWrap = document.createElement("div");
  tableWrap.className = "flex-1 overflow-auto border border-slate-200 rounded";

  const table = document.createElement("table");
  table.className = "w-full text-sm border-collapse";

  // Header
  const thead = document.createElement("thead");
  const headerRow = document.createElement("tr");
  for (const col of def.columns) {
    const th = document.createElement("th");
    th.className =
      "px-3 py-2 text-left text-xs font-semibold text-slate-600 bg-slate-50 border-b border-slate-200 whitespace-nowrap sticky top-0";
    if (col.width) th.style.width = col.width + "px";
    th.textContent = col.label;
    headerRow.appendChild(th);
  }
  thead.appendChild(headerRow);
  table.appendChild(thead);

  // Body
  const collData = data.collections?.[def.id];
  const rows = collData?.rows ?? [];

  const tbody = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    tr.className = "border-b border-slate-100 hover:bg-blue-50/40 transition-colors";

    if (def.detail_screen) {
      tr.classList.add("cursor-pointer");
      tr.addEventListener("dblclick", () => {
        _callbacks.onNavigate?.(def.detail_screen!, row);
      });
    }

    for (const col of def.columns) {
      const td = document.createElement("td");
      td.className = "px-3 py-1.5 whitespace-nowrap";

      const val = row[col.id];
      if (col.field_type === "number" && typeof val === "number") {
        td.className += " text-right tabular-nums";
        td.textContent = val.toLocaleString("ru-RU");
      } else if (col.id === "status") {
        const badge = document.createElement("span");
        const isDraft = String(val).toLowerCase().includes("черн") || String(val).toLowerCase() === "draft";
        badge.className = `inline-block px-2 py-0.5 text-xs font-medium rounded ${
          isDraft ? "bg-amber-100 text-amber-800" : "bg-emerald-100 text-emerald-800"
        }`;
        badge.textContent = String(val);
        td.appendChild(badge);
      } else {
        td.textContent = val != null ? String(val) : "";
      }

      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  tableWrap.appendChild(table);
  wrapper.appendChild(tableWrap);

  // Footer
  if (collData) {
    const footer = document.createElement("div");
    footer.className = "flex items-center justify-between mt-2 text-xs text-slate-400";
    footer.innerHTML = `<span>Записей: ${collData.total_count}</span>`;
    wrapper.appendChild(footer);
  }

  return wrapper;
}

// ─── Tabs ──────────────────────────────────────────────────────

function renderTabs(
  def: { id: string; tabs: Array<{ id: string; label: string; sections: SectionDef[] }> },
  data: DataPayload,
  behavior: BehaviorState,
): HTMLElement {
  const wrapper = document.createElement("div");
  wrapper.className = "flex flex-col flex-1";

  const tabBar = document.createElement("div");
  tabBar.className = "flex border-b border-slate-200 px-4";

  const content = document.createElement("div");
  content.className = "flex-1 flex flex-col";

  let activeIdx = 0;

  function activate(idx: number) {
    activeIdx = idx;
    tabBar.querySelectorAll("button").forEach((btn, i) => {
      btn.className = i === idx
        ? "px-4 py-2 text-sm font-medium text-accent border-b-2 border-accent -mb-px"
        : "px-4 py-2 text-sm text-slate-500 hover:text-slate-700";
    });
    content.innerHTML = "";
    for (const section of def.tabs[idx].sections) {
      content.appendChild(renderSection(section, data, behavior));
    }
  }

  for (let i = 0; i < def.tabs.length; i++) {
    const btn = document.createElement("button");
    btn.textContent = def.tabs[i].label;
    btn.addEventListener("click", () => activate(i));
    tabBar.appendChild(btn);
  }

  wrapper.appendChild(tabBar);
  wrapper.appendChild(content);
  activate(activeIdx);

  return wrapper;
}
