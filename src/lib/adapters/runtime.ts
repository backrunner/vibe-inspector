import { invoke } from '@tauri-apps/api/core';

export type RuntimeHealth = {
  app: string;
  tray: string;
  mcp: string;
  endpoint?: string;
  error?: string;
  loginItemEnabled: boolean;
  pid?: number;
  startedAt?: string;
  checkedAt: string;
};

export async function loadRuntimeHealth(): Promise<RuntimeHealth> {
  return invoke<RuntimeHealth>('runtime_health');
}

export async function updateLoginItem(enabled: boolean): Promise<RuntimeHealth> {
  return invoke<RuntimeHealth>('set_login_item', { input: { enabled } });
}

export function formatRuntimeTime(value: string | undefined): string {
  if (!value) return 'Unavailable';
  const numeric = Number(value);
  const date = Number.isFinite(numeric) ? new Date(numeric) : new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}
