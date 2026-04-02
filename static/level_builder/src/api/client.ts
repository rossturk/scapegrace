let toastFn: ((msg: string, type: string) => void) | null = null;

export function setToastHandler(fn: (msg: string, type: string) => void) {
  toastFn = fn;
}

function showToast(msg: string, type: string) {
  if (toastFn) toastFn(msg, type);
  else console.warn(`[${type}] ${msg}`);
}

export async function api<T = any>(
  url: string,
  options: { method?: string; body?: any; silent?: boolean } = {},
): Promise<T | null> {
  try {
    const res = await fetch(url, {
      method: options.method || 'GET',
      headers: options.body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
      body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || res.statusText);
    }
    if (res.status === 200 && res.headers.get('content-type')?.includes('json')) {
      return await res.json();
    }
    return null;
  } catch (err: any) {
    if (!options.silent) {
      showToast(`API error: ${err.message}`, 'error');
    }
    console.error('API request failed:', err);
    return null;
  }
}
