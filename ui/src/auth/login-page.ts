import { authenticate, type MockUser, type AuthError } from "./mock-users";

export function renderLoginPage(
  container: HTMLElement,
  onSuccess: (user: MockUser) => void,
): void {
  container.innerHTML = `
    <div class="h-full flex flex-col items-center justify-center bg-shell-bg">
      <div class="w-[380px] bg-white rounded-lg shadow-xl overflow-hidden">
        <div class="px-8 pt-8 pb-4 text-center">
          <h1 class="text-2xl font-bold text-slate-800 tracking-tight">ERP</h1>
          <p class="text-sm text-slate-400 mt-1">Вход в систему</p>
        </div>
        <form id="login-form" class="px-8 pb-8">
          <div class="mb-4">
            <label class="block text-sm font-medium text-slate-600 mb-1">Email</label>
            <input id="login-username" type="email" autocomplete="email"
              class="w-full px-3 py-2 border border-slate-300 rounded-md text-sm
                     focus:border-accent focus:ring-2 focus:ring-accent/20 outline-none transition-colors"
              placeholder="admin@erp.local" />
          </div>
          <div class="mb-5">
            <label class="block text-sm font-medium text-slate-600 mb-1">Пароль</label>
            <div class="relative">
              <input id="login-password" type="password" autocomplete="current-password"
                class="w-full px-3 py-2 border border-slate-300 rounded-md text-sm
                       focus:border-accent focus:ring-2 focus:ring-accent/20 outline-none transition-colors pr-10" />
              <button type="button" id="login-toggle-pw"
                class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 p-1"
                tabindex="-1" title="Показать пароль">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/>
                  <circle cx="12" cy="12" r="3"/>
                </svg>
              </button>
            </div>
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
  const usernameInput = document.getElementById("login-username") as HTMLInputElement;
  const passwordInput = document.getElementById("login-password") as HTMLInputElement;
  const errorEl = document.getElementById("login-error")!;
  const togglePw = document.getElementById("login-toggle-pw")!;

  usernameInput.focus();

  togglePw.addEventListener("click", () => {
    const isPassword = passwordInput.type === "password";
    passwordInput.type = isPassword ? "text" : "password";
  });

  form.addEventListener("submit", (e) => {
    e.preventDefault();
    errorEl.classList.add("hidden");

    const username = usernameInput.value.trim();
    const password = passwordInput.value;

    if (!username) {
      showError(errorEl, "Введите email");
      usernameInput.focus();
      return;
    }

    const result = authenticate(username, password);
    if ("message" in result) {
      showError(errorEl, (result as AuthError).message);
      passwordInput.value = "";
      passwordInput.focus();
      return;
    }

    onSuccess(result);
  });
}

function showError(el: HTMLElement, message: string): void {
  el.textContent = message;
  el.classList.remove("hidden");
}
