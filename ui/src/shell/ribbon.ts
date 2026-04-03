import type { ActionDef, BehaviorState } from "@/protocol/types";
import { icon } from "./icons";

export class Ribbon {
  private el: HTMLElement;
  private onAction: (actionId: string) => void;

  constructor(container: HTMLElement, onAction: (actionId: string) => void) {
    this.el = container;
    this.onAction = onAction;
  }

  render(actions: ActionDef[], behavior: BehaviorState): void {
    this.el.innerHTML = "";

    const mainGroup: ActionDef[] = [];
    const dangerGroup: ActionDef[] = [];

    for (const action of actions) {
      if (action.position !== "toolbar") continue;
      const state = behavior.actions[action.id];
      if (state && !state.visible) continue;

      if (action.group === "danger") {
        dangerGroup.push(action);
      } else {
        mainGroup.push(action);
      }
    }

    if (mainGroup.length > 0) {
      this.el.appendChild(this.renderGroup(mainGroup, behavior));
    }
    if (dangerGroup.length > 0) {
      const sep = document.createElement("div");
      sep.className = "w-px h-8 bg-ribbon-border mx-1";
      this.el.appendChild(sep);
      this.el.appendChild(this.renderGroup(dangerGroup, behavior, true));
    }
  }

  clear(): void {
    this.el.innerHTML = `<span class="text-sm text-slate-400 italic">Выберите вкладку</span>`;
  }

  private renderGroup(actions: ActionDef[], behavior: BehaviorState, danger = false): HTMLElement {
    const group = document.createElement("div");
    group.className = "flex items-center gap-1";

    for (const action of actions) {
      const state = behavior.actions[action.id];
      const enabled = !state || state.enabled;

      const btn = document.createElement("button");
      btn.className = [
        "flex items-center gap-1.5 px-3 py-1.5 rounded text-sm font-medium transition-colors",
        enabled
          ? danger
            ? "text-red-600 hover:bg-red-50"
            : "text-slate-700 hover:bg-slate-100"
          : "text-slate-300 cursor-not-allowed",
      ].join(" ");
      btn.disabled = !enabled;
      btn.title = !enabled && state?.reason ? state.reason : action.hotkey ?? "";

      let html = "";
      if (action.icon) html += icon(action.icon, 16);
      html += `<span>${action.label}</span>`;
      if (action.hotkey) {
        html += `<kbd class="hidden lg:inline text-xs text-slate-400 ml-1 px-1 border border-slate-200 rounded">${action.hotkey}</kbd>`;
      }
      btn.innerHTML = html;

      btn.addEventListener("click", () => {
        if (enabled) this.onAction(action.id);
      });

      group.appendChild(btn);
    }
    return group;
  }
}
