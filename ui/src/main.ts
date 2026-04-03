import "./styles/app.css";
import { renderLoginPage, type LoginResult } from "./auth/login-page";
import { createSession, restoreSession, clearSession, filterNavigation, type AuthSession } from "./auth/session";
import { OutlookBar } from "./shell/outlook-bar";
import { Ribbon } from "./shell/ribbon";
import { Mdi } from "./shell/mdi";
import type { MdiTab } from "./shell/mdi";
import { renderScreen } from "./engine/renderer";
import { mockNavigation, mockScreens, mockBehavior } from "./protocol/mock";
import { toast } from "./shell/toast";
import { icon } from "./shell/icons";
import { createProduct, getProduct, receiveGoods, getBalance } from "./protocol/api";
import type { DataPayload } from "./protocol/types";

const app = document.getElementById("app")!;

// ─── App lifecycle: LOGIN → SHELL ──────────────────────────────

function boot(): void {
  const session = restoreSession();
  if (session) {
    showShell(session);
  } else {
    showLogin();
  }
}

function showLogin(): void {
  renderLoginPage(app, (result: LoginResult) => {
    const session = createSession(result);
    showShell(session);
  });
}

function logout(): void {
  clearSession();
  showLogin();
}

// ─── Shell ─────────────────────────────────────────────────────

let ribbon: Ribbon;
let mdi: Mdi;
let tabCounter = 0;

function showShell(session: AuthSession): void {
  app.innerHTML = `
    <div class="h-full flex flex-col">
      <div class="h-12 flex items-center border-b border-ribbon-border bg-ribbon-bg px-4 gap-3" id="ribbon">
        <span class="text-sm text-slate-400 italic">Выберите экран</span>
      </div>
      <div class="flex-1 flex min-h-0">
        <aside class="w-52 flex flex-col bg-shell-bg border-r border-shell-border shrink-0" id="sidebar"></aside>
        <div class="flex-1 flex flex-col min-w-0 bg-mdi-bg">
          <div class="flex items-end gap-0 bg-slate-200 px-2 pt-1 min-h-[36px]" id="tab-bar"></div>
          <div class="flex-1 flex flex-col min-h-0 overflow-auto bg-white" id="mdi-content"></div>
        </div>
      </div>
      <div class="h-7 flex items-center px-4 bg-shell-bg text-shell-text text-xs border-t border-shell-border gap-4" id="status-bar">
        <span class="flex items-center gap-1">${icon("user", 12)} ${session.roles.join(", ")}</span>
        <span class="text-slate-500">|</span>
        <span class="font-mono text-slate-400">${session.tenant_id.substring(0, 8)}...</span>
        <button id="btn-logout" class="ml-auto flex items-center gap-1 text-slate-400 hover:text-white transition-colors">
          ${icon("log-out", 12)} Выйти
        </button>
      </div>
    </div>
  `;

  document.getElementById("btn-logout")!.addEventListener("click", logout);

  ribbon = new Ribbon(document.getElementById("ribbon")!, executeAction);

  mdi = new Mdi(
    document.getElementById("tab-bar")!,
    document.getElementById("mdi-content")!,
  );

  mdi.setOnTabChange((tab) => {
    if (tab) ribbon.render(tab.screen.actions, tab.behavior);
    else ribbon.clear();
  });

  mdi.setRenderContent((tab, container) => {
    renderScreen(tab.screen, tab.data, tab.behavior, container);
  });

  const filteredNav = filterNavigation(mockNavigation, session.roles);
  const sidebar = new OutlookBar(
    document.getElementById("sidebar")!,
    (screenId) => openScreen(screenId),
  );
  sidebar.render(filteredNav);
}

// ─── Open screen ───────────────────────────────────────────────

function openScreen(screenId: string): void {
  const screen = mockScreens[screenId];
  if (!screen) return;

  const tab: MdiTab = {
    id: `tab-${++tabCounter}`,
    screenId,
    title: screen.title,
    screen,
    data: {},
    behavior: mockBehavior[screenId] ?? { fields: {}, actions: {} },
  };
  mdi.openTab(tab);
}

// ─── Action handling → real API ────────────────────────────────

function getFormValues(): Record<string, string> {
  const values: Record<string, string> = {};
  document.querySelectorAll<HTMLInputElement>("#mdi-content input, #mdi-content textarea").forEach((el) => {
    if (el.id || el.name) {
      values[el.id || el.name] = el.value;
    }
  });
  return values;
}

async function executeAction(actionId: string): Promise<void> {
  const tab = mdi.getActiveTab();
  if (!tab) return;

  const key = `${tab.screenId}::${actionId}`;
  const vals = getFormValues();

  switch (key) {
    case "catalog.create_product::create": {
      const result = await createProduct({
        sku: vals["sku"] ?? "",
        name: vals["name"] ?? "",
        category: vals["category"] ?? "",
        unit: vals["unit"] ?? "",
      });
      if (result.ok) {
        toast("success", `Продукт создан: ${result.data.product_id}`);
      } else {
        toast("error", result.error);
      }
      break;
    }

    case "catalog.find_product::search": {
      const result = await getProduct(vals["sku"] ?? "");
      if (result.ok) {
        const data: DataPayload = {
          record: {
            sku: vals["sku"],
            product_id: result.data.product_id,
            sku_result: result.data.sku,
            name: result.data.name,
            category: result.data.category,
            unit: result.data.unit,
          },
        };
        mdi.updateActiveTab({ data });
        toast("success", `Найден: ${result.data.name}`);
      } else {
        toast("error", result.error);
      }
      break;
    }

    case "warehouse.receive::receive": {
      const result = await receiveGoods({
        sku: vals["sku"] ?? "",
        quantity: Number(vals["quantity"]) || 0,
      });
      if (result.ok) {
        const data: DataPayload = {
          record: {
            sku: vals["sku"],
            quantity: vals["quantity"],
            doc_number: result.data.doc_number,
            new_balance: result.data.new_balance,
            item_id: result.data.item_id,
            movement_id: result.data.movement_id,
          },
        };
        mdi.updateActiveTab({ data });
        toast("success", `Принято. Документ: ${result.data.doc_number}`);
      } else {
        toast("error", result.error);
      }
      break;
    }

    case "warehouse.balance::search": {
      const result = await getBalance(vals["sku"] ?? "");
      if (result.ok) {
        const data: DataPayload = {
          record: {
            sku: vals["sku"],
            sku_result: result.data.sku,
            balance: result.data.balance,
            product_name: result.data.product_name ?? "—",
            item_id: result.data.item_id ?? "—",
          },
        };
        mdi.updateActiveTab({ data });
        toast("success", `Остаток: ${result.data.balance}`);
      } else {
        toast("error", result.error);
      }
      break;
    }

    default:
      toast("info", `Unknown action: ${key}`);
  }
}

// ─── Start ─────────────────────────────────────────────────────

boot();
