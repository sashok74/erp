import type { NavigationDescriptor } from "@/protocol/types";
import type { LoginResult } from "./login-page";
import { api } from "@/protocol/api";

export interface AuthSession {
  user_id: string;
  tenant_id: string;
  roles: string[];
  token: string;
}

const SESSION_KEY = "erp_session";

export function createSession(login: LoginResult): AuthSession {
  const session: AuthSession = {
    user_id: login.user_id,
    tenant_id: login.tenant_id,
    roles: login.roles,
    token: login.token,
  };
  sessionStorage.setItem(SESSION_KEY, JSON.stringify(session));
  api.setToken(session.token);
  return session;
}

export function restoreSession(): AuthSession | null {
  const raw = sessionStorage.getItem(SESSION_KEY);
  if (!raw) return null;
  try {
    const session = JSON.parse(raw) as AuthSession;
    api.setToken(session.token);
    return session;
  } catch {
    return null;
  }
}

export function clearSession(): void {
  sessionStorage.removeItem(SESSION_KEY);
}

// ─── Navigation filtering by role ──────────────────────────────

const navAccess: Record<string, string[]> = {
  catalog: ["admin", "catalog_manager", "viewer"],
  warehouse: ["admin", "warehouse_manager", "warehouse_operator", "viewer"],
};

export function filterNavigation(nav: NavigationDescriptor, roles: string[]): NavigationDescriptor {
  return {
    groups: nav.groups.filter((group) => {
      const allowed = navAccess[group.id];
      if (!allowed) return true;
      return roles.some((r) => allowed.includes(r));
    }),
  };
}
