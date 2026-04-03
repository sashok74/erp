import type { MockUser, UserTenant } from "./mock-users";

const roleLabels: Record<string, string> = {
  admin: "Администратор",
  viewer: "Наблюдатель",
  catalog_manager: "Менеджер каталога",
  warehouse_manager: "Менеджер склада",
  warehouse_operator: "Кладовщик",
};

export function renderTenantPicker(
  container: HTMLElement,
  user: MockUser,
  onSelect: (tenant: UserTenant) => void,
  onLogout: () => void,
): void {
  container.innerHTML = `
    <div class="h-full flex flex-col items-center justify-center bg-shell-bg">
      <div class="w-[420px] bg-white rounded-lg shadow-xl overflow-hidden">
        <div class="px-8 pt-8 pb-4">
          <h2 class="text-lg font-semibold text-slate-800">Выберите организацию</h2>
          <p class="text-sm text-slate-400 mt-1">Доступно организаций: ${user.tenants.length}</p>
        </div>
        <div id="tenant-list" class="px-4 pb-4"></div>
        <div class="px-8 pb-6 flex items-center justify-between text-sm border-t border-slate-100 pt-4">
          <span class="text-slate-500">Вы вошли как <span class="font-medium text-slate-700">${user.display_name}</span></span>
          <button id="tenant-logout" class="text-accent hover:underline">Выйти</button>
        </div>
      </div>
      <p class="text-xs text-slate-500 mt-6">ERP UI Engine v0.1</p>
    </div>
  `;

  const list = document.getElementById("tenant-list")!;

  for (const tenant of user.tenants) {
    const btn = document.createElement("button");
    btn.className =
      "w-full flex items-start gap-3 px-4 py-3 rounded-lg hover:bg-slate-50 transition-colors text-left mb-1";

    const rolesText = tenant.roles.map((r) => roleLabels[r] ?? r).join(", ");

    btn.innerHTML = `
      <div class="w-9 h-9 rounded-lg bg-accent/10 text-accent flex items-center justify-center shrink-0 mt-0.5">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M6 22V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v18Z"/><path d="M6 12H4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2"/><path d="M18 9h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-2"/><path d="M10 6h4"/><path d="M10 10h4"/><path d="M10 14h4"/><path d="M10 18h4"/>
        </svg>
      </div>
      <div class="flex-1 min-w-0">
        <div class="font-medium text-slate-800">${tenant.tenant_name}</div>
        <div class="text-xs text-slate-400 mt-0.5">${tenant.tenant_slug} · ${rolesText}</div>
      </div>
      <svg class="text-slate-300 mt-2 shrink-0" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="m9 18 6-6-6-6"/>
      </svg>
    `;

    btn.addEventListener("click", () => onSelect(tenant));
    list.appendChild(btn);
  }

  document.getElementById("tenant-logout")!.addEventListener("click", onLogout);
}
