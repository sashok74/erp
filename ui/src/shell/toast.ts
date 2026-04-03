const COLORS = {
  success: "bg-emerald-600",
  error: "bg-red-600",
  warning: "bg-amber-500",
  info: "bg-slate-600",
} as const;

let container: HTMLElement | null = null;

function getContainer(): HTMLElement {
  if (container) return container;
  container = document.createElement("div");
  container.className = "fixed top-3 right-3 z-50 flex flex-col gap-2 pointer-events-none";
  document.body.appendChild(container);
  return container;
}

export function toast(level: keyof typeof COLORS, message: string): void {
  const el = document.createElement("div");
  el.className = `${COLORS[level]} text-white text-sm px-4 py-2.5 rounded shadow-lg pointer-events-auto
    transform translate-x-full opacity-0 transition-all duration-300`;
  el.textContent = message;

  getContainer().appendChild(el);

  requestAnimationFrame(() => {
    el.classList.remove("translate-x-full", "opacity-0");
  });

  setTimeout(() => {
    el.classList.add("translate-x-full", "opacity-0");
    setTimeout(() => el.remove(), 300);
  }, 2500);
}
