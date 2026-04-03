export interface MockUser {
  id: string;
  username: string;
  password: string;
  display_name: string;
  email: string;
  is_active: boolean;
  tenants: UserTenant[];
}

export interface UserTenant {
  tenant_id: string;
  tenant_name: string;
  tenant_slug: string;
  roles: string[];
}

const T = {
  DEMO: { tenant_id: "019d0001-0000-7000-8000-000000000001", tenant_name: "Демо Компания", tenant_slug: "demo" },
  STROY: { tenant_id: "019d0001-0000-7000-8000-000000000002", tenant_name: "СтройМонтаж", tenant_slug: "stroymontazh" },
  TEST: { tenant_id: "019d0001-0000-7000-8000-000000000003", tenant_name: "Тест (архив)", tenant_slug: "test-archive" },
};

export const mockUsers: MockUser[] = [
  {
    id: "u-001", username: "admin", password: "admin",
    display_name: "Администратор А.А.", email: "admin@erp.local", is_active: true,
    tenants: [
      { ...T.DEMO, roles: ["admin"] },
      { ...T.STROY, roles: ["admin"] },
      { ...T.TEST, roles: ["admin"] },
    ],
  },
  {
    id: "u-002", username: "ivanov", password: "ivanov",
    display_name: "Иванов И.И.", email: "ivanov@erp.local", is_active: true,
    tenants: [
      { ...T.DEMO, roles: ["warehouse_manager"] },
    ],
  },
  {
    id: "u-003", username: "petrova", password: "petrova",
    display_name: "Петрова М.С.", email: "petrova@erp.local", is_active: true,
    tenants: [
      { ...T.DEMO, roles: ["catalog_manager", "warehouse_operator"] },
      { ...T.STROY, roles: ["catalog_manager"] },
    ],
  },
  {
    id: "u-004", username: "sidorov", password: "sidorov",
    display_name: "Сидоров К.В.", email: "sidorov@erp.local", is_active: true,
    tenants: [
      { ...T.DEMO, roles: ["viewer"] },
    ],
  },
  {
    id: "u-005", username: "smirnova", password: "smirnova",
    display_name: "Смирнова Е.Л.", email: "smirnova@erp.local", is_active: true,
    tenants: [
      { ...T.STROY, roles: ["admin"] },
    ],
  },
  {
    id: "u-006", username: "inactive", password: "inactive",
    display_name: "Неактивный П.П.", email: "inactive@erp.local", is_active: false,
    tenants: [],
  },
];

export interface AuthError {
  message: string;
}

export function authenticate(email: string, password: string): MockUser | AuthError {
  const user = mockUsers.find((u) => u.email === email);
  if (!user || user.password !== password) {
    return { message: "Неверный email или пароль" };
  }
  if (!user.is_active) {
    return { message: "Учётная запись деактивирована" };
  }
  return user;
}
