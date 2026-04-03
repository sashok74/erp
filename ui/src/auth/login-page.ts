import { devToken, api } from "@/protocol/api";

const KNOWN_ROLES = [
  { code: "admin", label: "Администратор" },
  { code: "warehouse_manager", label: "Менеджер склада" },
  { code: "warehouse_operator", label: "Кладовщик" },
  { code: "catalog_manager", label: "Менеджер каталога" },
  { code: "viewer", label: "Наблюдатель" },
];

export interface LoginResult {
  token: string;
  user_id: string;
  tenant_id: string;
  roles: string[];
}

export function renderLoginPage(
  container: HTMLElement,
  onSuccess: (result: LoginResult) => void,
): void {
  const roleCheckboxes = KNOWN_ROLES.map((r) =>
    `<label class="flex items-center gap-2 text-sm text-slate-600">
      <input type="checkbox" name="roles" value="${r.code}" class="rounded border-slate-300 text-accent focus:ring-accent"
        ${r.code === "admin" ? "checked" : ""} />
      ${r.label}
    </label>`
  ).join("");

  container.innerHTML = `
    <div class="h-full flex flex-col items-center justify-center bg-shell-bg">
      <div class="w-[400px] bg-white rounded-lg shadow-xl overflow-hidden">
        <div class="px-8 pt-8 pb-4 text-center">
          <h1 class="text-2xl font-bold text-slate-800 tracking-tight">ERP</h1>
          <p class="text-sm text-slate-400 mt-1">Dev Mode — вход через /dev/token</p>
        </div>
        <form id="login-form" class="px-8 pb-8">
          <div class="mb-4">
            <label class="block text-sm font-medium text-slate-600 mb-1">Tenant ID</label>
            <input id="login-tenant" type="text"
              class="w-full px-3 py-2 border border-slate-300 rounded-md text-sm font-mono
                     focus:border-accent focus:ring-2 focus:ring-accent/20 outline-none transition-colors"
              placeholder="UUID тенанта из базы" />
            <p class="text-xs text-slate-400 mt-1">SELECT id FROM common.tenants;</p>
          </div>
          <div class="mb-5">
            <label class="block text-sm font-medium text-slate-600 mb-2">Роли</label>
            <div class="flex flex-col gap-1.5">${roleCheckboxes}</div>
          </div>
          <div id="login-error" class="mb-4 text-sm text-red-600 hidden"></div>
          <button type="submit"
            class="w-full py-2.5 bg-accent hover:bg-accent-hover text-white font-medium rounded-md
                   transition-colors text-sm">
            Войти
          </button>
        </form>
      </div>
      <p class="text-xs text-slate-500 mt-6">ERP UI Engine v0.1</p>
    </div>
  `;

  const form = document.getElementById("login-form") as HTMLFormElement;
  const tenantInput = document.getElementById("login-tenant") as HTMLInputElement;
  const errorEl = document.getElementById("login-error")!;

  tenantInput.focus();

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    errorEl.classList.add("hidden");

    const tenantId = tenantInput.value.trim();
    if (!tenantId) {
      showError(errorEl, "Введите Tenant ID");
      return;
    }

    const checkboxes = form.querySelectorAll<HTMLInputElement>('input[name="roles"]:checked');
    const roles = Array.from(checkboxes).map((cb) => cb.value);
    if (roles.length === 0) {
      showError(errorEl, "Выберите хотя бы одну роль");
      return;
    }

    const result = await devToken({ tenant_id: tenantId, roles });

    if (!result.ok) {
      showError(errorEl, result.error);
      return;
    }

    api.setToken(result.data.token);
    onSuccess({ ...result.data, roles });
  });
}

function showError(el: HTMLElement, message: string): void {
  el.textContent = message;
  el.classList.remove("hidden");
}
