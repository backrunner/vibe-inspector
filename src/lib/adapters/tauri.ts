import { invoke } from '@tauri-apps/api/core';

export type RuntimeHealth = {
  app: 'desktop';
  tray: 'online' | 'offline';
  mcp: 'starting' | 'online' | 'offline' | 'error';
  endpoint?: string | null;
  error?: string | null;
  checkedAt: string;
};

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function getRuntimeHealth(): Promise<RuntimeHealth> {
  if (!isTauriRuntime()) {
    return { app: 'desktop', tray: 'offline', mcp: 'offline', checkedAt: new Date().toISOString() };
  }
  return invoke<RuntimeHealth>('runtime_health');
}
