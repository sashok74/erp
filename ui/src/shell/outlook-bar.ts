import type { NavigationDescriptor, NavGroup } from "@/protocol/types";
import { icon } from "./icons";

export class OutlookBar {
  private el: HTMLElement;
  private activeGroupId: string | null = null;
  private onNavigate: (screenId: string) => void;

  constructor(container: HTMLElement, onNavigate: (screenId: string) => void) {
    this.el = container;
    this.onNavigate = onNavigate;
  }

  render(nav: NavigationDescriptor): void {
    this.el.innerHTML = "";

    const header = document.createElement("div");
    header.className = "h-12 flex items-center justify-center border-b border-shell-border";
    header.innerHTML = `<span class="text-base font-bold tracking-wide text-white">ERP</span>`;
    this.el.appendChild(header);

    const groups = document.createElement("div");
    groups.className = "flex-1 overflow-y-auto py-2";
    this.el.appendChild(groups);

    for (const group of nav.groups) {
      groups.appendChild(this.renderGroup(group));
    }

    // Auto-expand first group
    if (nav.groups.length > 0) {
      this.toggleGroup(nav.groups[0].id);
    }
  }

  private renderGroup(group: NavGroup): HTMLElement {
    const wrapper = document.createElement("div");
    wrapper.dataset.groupId = group.id;

    const btn = document.createElement("button");
    btn.className =
      "w-full flex items-center gap-2 px-3 py-2.5 text-shell-text hover:bg-shell-hover transition-colors text-sm font-medium";
    btn.innerHTML = `${icon(group.icon, 18)}<span>${group.label}</span>
      <span class="ml-auto transition-transform duration-200" data-chevron>${icon("chevron-down", 14)}</span>`;
    btn.addEventListener("click", () => this.toggleGroup(group.id));
    wrapper.appendChild(btn);

    const items = document.createElement("div");
    items.className = "overflow-hidden max-h-0 transition-all duration-200";
    items.dataset.items = group.id;
    for (const item of group.items) {
      const link = document.createElement("button");
      link.className =
        "w-full flex items-center gap-2 pl-9 pr-3 py-1.5 text-sm text-slate-400 hover:text-white hover:bg-shell-hover transition-colors";
      link.textContent = item.label;
      link.addEventListener("click", () => this.onNavigate(item.screen_id));
      items.appendChild(link);
    }
    wrapper.appendChild(items);

    return wrapper;
  }

  private toggleGroup(groupId: string): void {
    const isOpen = this.activeGroupId === groupId;
    this.activeGroupId = isOpen ? null : groupId;

    this.el.querySelectorAll("[data-items]").forEach((el) => {
      const items = el as HTMLElement;
      const chevron = items.previousElementSibling?.querySelector("[data-chevron]") as HTMLElement | null;
      if (items.dataset.items === this.activeGroupId) {
        items.style.maxHeight = items.scrollHeight + "px";
        chevron?.classList.add("rotate-180");
      } else {
        items.style.maxHeight = "0";
        chevron?.classList.remove("rotate-180");
      }
    });
  }
}
