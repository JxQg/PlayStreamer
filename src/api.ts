import { invoke, isTauri } from '@tauri-apps/api/core';

export const desktop = isTauri();
export type Theme = 'system' | 'light' | 'dark';
export interface AppConfig {
  networkInterfaceId: string;
  ps5Ip: string;
  upstreamDns: string[];
  theme: Theme;
  setupCompleted: boolean;
}
export interface NetworkInterface {
  id: string; name: string; ipv4: string; prefixLength: number; dnsServers: string[]; eligible: boolean;
}
export type ServiceStatus = 'idle' | 'starting' | 'waiting' | 'receiving' | 'dnsOnly' | 'networkChanged' | 'mediaError' | 'suspended';
export interface Snapshot {
  status: ServiceStatus; message: string; lanIp: string | null; dnsRunning: boolean; playbackUrl: string;
  stream: { bitrateKbps: number; tracks: string[]; connectedSeconds: number; readers: number };
  lastError: string | null; dnsQueries: number; dnsIntercepts: number;
}
export interface DiagnosticReport { checks: { id: string; title: string; status: string; detail: string }[] }
export interface UpdateInfo { currentVersion: string; latestVersion: string; releaseUrl: string; updateAvailable: boolean }

export async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) throw new Error('请在 PlayStreamer 桌面客户端中使用此功能。');
  return invoke<T>(name, args);
}
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
