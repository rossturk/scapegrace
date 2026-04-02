import { useEffect, useRef } from 'preact/hooks';
import { signal } from '@preact/signals';
import { setToastHandler } from '../api/client';

interface ToastItem {
  id: number;
  msg: string;
  type: string;
}

const toasts = signal<ToastItem[]>([]);
let nextId = 0;

export function showToast(msg: string, type = 'info') {
  const id = nextId++;
  toasts.value = [...toasts.value, { id, msg, type }];
  setTimeout(() => {
    toasts.value = toasts.value.filter(t => t.id !== id);
  }, 4000);
}

// Wire up API client toast handler
setToastHandler(showToast);

export function ToastContainer() {
  return (
    <div class="toast-container">
      {toasts.value.map(t => (
        <div key={t.id} class={`toast ${t.type}`}>{t.msg}</div>
      ))}
    </div>
  );
}
