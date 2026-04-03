import "./styles/app.css";
import { renderLoginPage } from "./auth/login-page";
import { renderTenantPicker } from "./auth/tenant-picker";
import { createSession, restoreSession, clearSession, filterNavigation, type AuthSession } from "./auth/session";
import type { MockUser } from "./auth/mock-users";
import { OutlookBar } from "./shell/outlook-bar";
import { Ribbon } from "./shell/ribbon";
import { Mdi } from "./shell/mdi";
import type { MdiTab } from "./shell/mdi";
import { renderScreen } from "./engine/renderer";
import { mockNavigation, mockScreens, mockData, mockBehavior } from "./protocol/mock";
import { handleAction } from "./protocol/mock-server";
import { toast } from "./shell/toast";
import { icon } from "./shell/icons";
import type { UiEffect, DataPayload } from "./protocol/types";

const app = document.getElementById("app")!;

// ─── App lifecycle: LOGIN → TENANT_PICKER → SHELL ─────────────

function boot(): void {
  const session = restoreSession();
  if (session) {
    showShell(session);
  } else {
    showLogin();
  }
}

function showLogin(): void {
  renderLoginPage(app, onLoginSuccess);
}

function onLoginSuccess(user: MockUser): void {
  if (user.tenants.length === 1) {
    const session = createSession(user, user.tenants[0]);
    showShell(session);
  } else {
    renderTenantPicker(app, user, (tenant) => {
      const session = createSession(user, tenant);
      showShell(session);
    }, () => showLogin());
  }
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
        <span class="text-sm text-slate-400 italic">Выберите вкладку</span>
      </div>
      <div class="flex-1 flex min-h-0">
        <aside class="w-52 flex flex-col bg-shell-bg border-r border-shell-border shrink-0" id="sidebar"></aside>
        <div class="flex-1 flex flex-col min-w-0 bg-mdi-bg">
          <div class="flex items-end gap-0 bg-slate-200 px-2 pt-1 min-h-[36px]" id="tab-bar"></div>
          <div class="flex-1 flex flex-col min-h-0 overflow-auto bg-white" id="mdi-content"></div>
        </div>
      </div>
      <div class="h-7 flex items-center px-4 bg-shell-bg text-shell-text text-xs border-t border-shell-border gap-4" id="status-bar">
        <span class="flex items-center gap-1">${icon("user", 12)} ${session.user.display_name}</span>
        <span class="text-slate-500">|</span>
        <span>${session.tenant.name}</span>
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
    if (tab) {
      ribbon.render(tab.screen.actions, tab.behavior);
    } else {
      ribbon.clear();
    }
  });

  mdi.setRenderContent((tab, container) => {
    renderScreen(tab.screen, tab.data, tab.behavior, container, {
      onNavigate: openScreen,
    });
  });

  const filteredNav = filterNavigation(mockNavigation, session.roles);
  const sidebar = new OutlookBar(
    document.getElementById("sidebar")!,
    (screenId) => openScreen(screenId),
  );
  sidebar.render(filteredNav);
}

// ─── Effect handler (single entry point for HTTP and future WS) ─

function applyEffects(effects: UiEffect[]): void {
  for (const effect of effects) {
    switch (effect.type) {
      case "toast":
        toast(effect.level, effect.message);
        break;
      case "close_form":
        mdi.closeActiveTab();
        return;
      case "navigate":
        openScreen(effect.screen_id);
        break;
      case "update_behavior":
        mdi.updateActiveTab({ behavior: effect.behavior });
        break;
      case "update_data":
        mdi.updateActiveTab({ data: effect.data as DataPayload });
        break;
      case "refresh":
        break;
    }
  }
}

// ─── Action handling (the behavior cycle) ──────────────────────

function executeAction(actionId: string): void {
  const tab = mdi.getActiveTab();
  if (!tab) return;

  const action = tab.screen.actions.find((a) => a.id === actionId);
  if (action?.confirm) {
    if (!confirm(action.confirm)) return;
  }

  const result = handleAction(tab.screenId, actionId, tab.data);
  applyEffects(result.effects);

  if (result.data || result.behavior) {
    mdi.updateActiveTab({
      data: result.data,
      behavior: result.behavior,
    });
  }
}

function openScreen(screenId: string, _row?: Record<string, unknown>): void {
  const screen = mockScreens[screenId];
  if (!screen) return;

  const tab: MdiTab = {
    id: `tab-${++tabCounter}`,
    screenId,
    title: screen.title,
    screen,
    data: mockData[screenId] ?? { collections: {} },
    behavior: mockBehavior[screenId] ?? { fields: {}, actions: {} },
  };
  mdi.openTab(tab);
}

// ─── Start ─────────────────────────────────────────────────────

boot();
