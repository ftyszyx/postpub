const API_BASE = window.__POSTPUB_API_BASE__ || "";

export interface ApiResponse<T> {
  success: boolean;
  data: T;
  message?: string;
}

export interface ErrorResponse {
  success: false;
  error: string;
}

type QueryValue = string | number | boolean | null | undefined;

interface RequestOptions {
  method?: string;
  body?: unknown;
  query?: Record<string, QueryValue>;
}

function isAbsoluteHttpUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
}

export function encodePathSegments(value: string): string {
  return value
    .split("/")
    .filter((segment) => segment.length > 0)
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

export function buildUrl(path: string, query?: Record<string, QueryValue>): string {
  let url: URL;

  if (!API_BASE) {
    url = new URL(path, window.location.origin);
  } else if (isAbsoluteHttpUrl(API_BASE)) {
    const base = API_BASE.endsWith("/") ? API_BASE : `${API_BASE}/`;
    url = new URL(path.replace(/^\//, ""), base);
  } else {
    const prefix = API_BASE.endsWith("/") ? API_BASE.slice(0, -1) : API_BASE;
    url = new URL(`${prefix}${path}`, window.location.origin);
  }

  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value === undefined || value === null || value === "") {
        continue;
      }
      url.searchParams.set(key, String(value));
    }
  }

  if (isAbsoluteHttpUrl(API_BASE)) {
    return url.toString();
  }

  return `${url.pathname}${url.search}`;
}

export async function apiGet<T>(path: string, query?: Record<string, QueryValue>): Promise<T> {
  return request<T>(path, { method: "GET", query });
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return request<T>(path, { method: "POST", body });
}

export async function apiPut<T>(path: string, body?: unknown): Promise<T> {
  return request<T>(path, { method: "PUT", body });
}

export async function apiDelete<T>(path: string): Promise<T> {
  return request<T>(path, { method: "DELETE" });
}

async function request<T>(path: string, options: RequestOptions): Promise<T> {
  const response = await fetch(buildUrl(path, options.query), {
    method: options.method || "GET",
    credentials: "same-origin",
    headers: options.body ? { "Content-Type": "application/json" } : undefined,
    body: options.body ? JSON.stringify(options.body) : undefined
  });

  if (!response.ok) {
    throw new Error(await readError(response));
  }

  return response.json() as Promise<T>;
}

async function readError(response: Response): Promise<string> {
  try {
    const data = (await response.json()) as Partial<ErrorResponse>;
    return data.error || response.statusText;
  } catch {
    return response.statusText;
  }
}
