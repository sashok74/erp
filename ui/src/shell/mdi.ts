import type { ScreenDescriptor, DataPayload, BehaviorState } from "@/protocol/types";
import { icon } from "./icons";

export interface MdiTab {
  id: string;
  screenId: string;
  title: string;
  screen: ScreenDescriptor;
  data: DataPayload;
  behavior: BehaviorState;
}

export class Mdi {
  private tabBarEl: HTMLElement;
  private contentEl: HTMLElement;
  private tabs: MdiTab[] = [];
  private activeTabId: string | null = null;
  private onTabChange: ((tab: MdiTab | null) => void) | null = null;
  private renderContent: ((tab: MdiTab, container: HTMLElement) => void) | null = null;

  constructor(tabBar: HTMLElement, content: HTMLElement) {
    this.tabBarEl = tabBar;
    this.contentEl = content;
  }

  setOnTabChange(cb: (tab: MdiTab | null) => void): void {
    this.onTabChange = cb;
  }

  setRenderContent(cb: (tab: MdiTab, container: HTMLElement) => void): void {
    this.renderContent = cb;
  }

  openTab(tab: MdiTab): void {
    const existing = this.tabs.find((t) => t.screenId === tab.screenId);
    if (existing) {
      this.activateTab(existing.id);
      return;
    }
    this.tabs.push(tab);
    this.activateTab(tab.id);
  }

  closeTab(tabId: string): void {
    const idx = this.tabs.findIndex((t) => t.id === tabId);
    if (idx === -1) return;
    this.tabs.splice(idx, 1);

    if (this.activeTabId === tabId) {
      const next = this.tabs[Math.min(idx, this.tabs.length - 1)] ?? null;
      this.activeTabId = next?.id ?? null;
    }
    this.renderTabs();
    this.renderActiveContent();
    this.onTabChange?.(this.getActiveTab());
  }

  getActiveTab(): MdiTab | null {
    return this.tabs.find((t) => t.id === this.activeTabId) ?? null;
  }

  /** Update active tab's state and re-render */
  updateActiveTab(patch: { data?: DataPayload; behavior?: BehaviorState }): void {
    const tab = this.getActiveTab();
    if (!tab) return;
    if (patch.data) tab.data = patch.data;
    if (patch.behavior) tab.behavior = patch.behavior;
    this.renderActiveContent();
    this.onTabChange?.(tab);
  }

  closeActiveTab(): void {
    if (this.activeTabId) this.closeTab(this.activeTabId);
  }

  private activateTab(tabId: string): void {
    this.activeTabId = tabId;
    this.renderTabs();
    this.renderActiveContent();
    this.onTabChange?.(this.getActiveTab());
  }

  private renderTabs(): void {
    this.tabBarEl.innerHTML = "";

    for (const tab of this.tabs) {
      const isActive = tab.id === this.activeTabId;
      const el = document.createElement("button");
      el.className = [
        "group flex items-center gap-1.5 pl-3 pr-1.5 py-1.5 text-sm border-t-2 transition-colors whitespace-nowrap max-w-[200px]",
        isActive
          ? "bg-white border-accent text-slate-900 font-medium"
          : "bg-slate-100 border-transparent text-slate-500 hover:text-slate-700 hover:bg-slate-50",
      ].join(" ");

      const label = document.createElement("span");
      label.className = "truncate";
      label.textContent = tab.title;
      label.addEventListener("click", () => this.activateTab(tab.id));
      el.appendChild(label);

      const close = document.createElement("span");
      close.className =
        "ml-1 p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-slate-200 transition-opacity cursor-pointer";
      close.innerHTML = icon("x", 12);
      close.addEventListener("click", (e) => {
        e.stopPropagation();
        this.closeTab(tab.id);
      });
      el.appendChild(close);

      this.tabBarEl.appendChild(el);
    }
  }

  private renderActiveContent(): void {
    this.contentEl.innerHTML = "";

    const tab = this.getActiveTab();
    if (!tab) {
      this.contentEl.innerHTML = `
        <div class="flex items-center justify-center h-full text-slate-400">
          <div class="text-center">
            <div class="text-5xl mb-3 opacity-30">☰</div>
            <div class="text-lg">Выберите пункт в навигации</div>
          </div>
        </div>`;
      return;
    }

    if (this.renderContent) {
      this.renderContent(tab, this.contentEl);
    }
  }
}
