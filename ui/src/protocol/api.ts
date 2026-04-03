class ApiClient {
  private token: string | null = null;

  setToken(token: string): void {
    this.token = token;
  }

  getToken(): string | null {
    return this.token;
  }

  async get<T = unknown>(path: string, params?: Record<string, string>): Promise<ApiResult<T>> {
    const url = params
      ? `${path}?${new URLSearchParams(params)}`
      : path;
    return this.request<T>(url, { method: "GET" });
  }

  async post<T = unknown>(path: string, body: unknown): Promise<ApiResult<T>> {
    return this.request<T>(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  }

  private async request<T>(path: string, init: RequestInit): Promise<ApiResult<T>> {
    const headers: Record<string, string> = {
      ...(init.headers as Record<string, string> ?? {}),
    };
    if (this.token) {
      headers["Authorization"] = `Bearer ${this.token}`;
    }

    try {
      const res = await fetch(path, { ...init, headers });
      const data = await res.json();

      if (!res.ok) {
        const msg = data?.error?.message ?? data?.error ?? `HTTP ${res.status}`;
        return { ok: false, error: String(msg), status: res.status };
      }

      return { ok: true, data: data as T };
    } catch (e) {
      return { ok: false, error: String(e), status: 0 };
    }
  }
}

export type ApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: string; status: number };

export const api = new ApiClient();

// ─── Typed API calls ───────────────────────────────────────────

export interface DevTokenRequest {
  tenant_id: string;
  roles: string[];
}

export interface DevTokenResponse {
  token: string;
  user_id: string;
  tenant_id: string;
}

export async function devToken(req: DevTokenRequest): Promise<ApiResult<DevTokenResponse>> {
  return api.post("/dev/token", req);
}

export interface CreateProductRequest {
  sku: string;
  name: string;
  category: string;
  unit: string;
}

export interface Product {
  product_id: string;
  sku: string;
  name: string;
  category: string;
  unit: string;
}

export async function createProduct(req: CreateProductRequest): Promise<ApiResult<{ product_id: string }>> {
  return api.post("/api/catalog/products", req);
}

export async function getProduct(sku: string): Promise<ApiResult<Product>> {
  return api.get("/api/catalog/products", { sku });
}

export interface ReceiveGoodsRequest {
  sku: string;
  quantity: number;
}

export interface ReceiveGoodsResult {
  item_id: string;
  movement_id: string;
  new_balance: string;
  doc_number: string;
}

export async function receiveGoods(req: ReceiveGoodsRequest): Promise<ApiResult<ReceiveGoodsResult>> {
  return api.post("/api/warehouse/receive", req);
}

export interface Balance {
  sku: string;
  balance: string;
  item_id: string | null;
  product_name: string | null;
}

export async function getBalance(sku: string): Promise<ApiResult<Balance>> {
  return api.get("/api/warehouse/balance", { sku });
}
