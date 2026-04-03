import type { MockUser, UserTenant } from "./mock-users";
import type { NavigationDescriptor } from "@/protocol/types";

export interface AuthSession {
  user: { id: string; display_name: string; email: string };
  tenant: { id: string; name: string; slug: string };
  roles: string[];
  token: string;
}

const SESSION_KEY = "erp_session";

export function createSession(user: MockUser, tenant: UserTenant): AuthSession {
  const session: AuthSession = {
    user: { id: user.id, display_name: user.display_name, email: user.email },
    tenant: { id: tenant.tenant_id, name: tenant.tenant_name, slug: tenant.tenant_slug },
    roles: tenant.roles,
    token: `mock-jwt-${user.id}-${tenant.tenant_id}`,
  };
  sessionStorage.setItem(SESSION_KEY, JSON.stringify(session));
  return session;
}

export function restoreSession(): AuthSession | null {
  const raw = sessionStorage.getItem(SESSION_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as AuthSession;
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
  admin: ["admin"],
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
