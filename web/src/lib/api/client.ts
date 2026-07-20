import { ApiError, type ApiEnvelope, type CacheBackend } from "../api";

type FlexibleEnvelope<T> =
  ApiEnvelope<T> | { ok: boolean; message?: string; data?: T; error?: string };

const API_BASE = import.meta.env.VITE_MCPSTORE_API_BASE || "/api";

export function apiUrl(path: string) {
  return `${API_BASE}${path}`;
}

export function buildQuery(
  params: Record<string, string | number | boolean | null | undefined>,
) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === "") continue;
    search.set(key, String(value));
  }
  const query = search.toString();
  return query ? `?${query}` : "";
}

export async function readJson<T>(response: Response): Promise<T> {
  const text = await response.text();
  const body = text ? JSON.parse(text) : null;
  if (!response.ok) {
    const message =
      body?.message || body?.errors?.[0]?.message || response.statusText;
    throw new ApiError(message, response.status);
  }
  return body as T;
}

export async function api<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Accept", "application/json");

  if (
    options.body !== undefined &&
    !(options.body instanceof FormData) &&
    !headers.has("Content-Type")
  ) {
    headers.set("Content-Type", "application/json");
  }

  const payload = await readJson<T | FlexibleEnvelope<T>>(
    await fetch(apiUrl(path), {
      ...options,
      headers,
    }),
  );

  if (payload && typeof payload === "object" && "success" in payload) {
    const envelope = payload as ApiEnvelope<T>;
    if (!envelope.success)
      throw new ApiError(
        envelope.errors?.[0]?.message || envelope.message,
        200,
      );
    return envelope.data as T;
  }

  if (payload && typeof payload === "object" && "ok" in payload) {
    const envelope = payload as {
      ok: boolean;
      message?: string;
      data?: T;
      error?: string;
    };
    if (!envelope.ok)
      throw new ApiError(
        envelope.error || envelope.message || "Request failed",
        200,
      );
    return envelope.data as T;
  }

  return payload as T;
}

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(apiUrl(path), {
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  });
  const payload = await readJson<ApiEnvelope<T>>(response);
  if (!payload.success) {
    throw new ApiError(
      payload.errors?.[0]?.message || payload.message,
      response.status,
    );
  }
  return payload.data as T;
}
