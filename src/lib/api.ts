import { invoke } from '@tauri-apps/api/core';

export type EdgeSide = 'top' | 'left' | 'right';
export type VrchatOscMode = 'clock' | 'music' | 'system' | 'agent' | 'custom';

export interface Settings {
  version: number;
  remote_url: string;
  remember_address: boolean;
  microphone_id: string;
  microphone_label: string;
  replace_clipboard_with_vr: boolean;
  delay_vr_capture: boolean;
  force_secure_context: boolean;
  auto_secure_context: boolean;
  window_opacity: number;
  boss_key: string;
  edge_autohide: boolean;
  edge_autohide_edges: EdgeSide[];
  reduce_motion: boolean;
  language: 'zh' | 'en';
  vrchat_osc_enabled: boolean;
  vrchat_osc_host: string;
  vrchat_osc_port: number;
  vrchat_osc_mode: VrchatOscMode;
  vrchat_osc_custom_text: string;
}

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  data_dir: string;
}

export interface LayoutState {
  mode: 'home' | 'remote';
  overlay: boolean;
  address: string;
  secure_context_applied: boolean;
  force_secure_context: boolean;
}

export interface Bootstrap {
  app: AppInfo;
  settings: Settings;
  layout: LayoutState;
  test_open_url?: string | null;
}

export const DEFAULT_SETTINGS: Settings = {
  version: 1,
  remote_url: '',
  remember_address: true,
  microphone_id: '',
  microphone_label: '',
  replace_clipboard_with_vr: false,
  delay_vr_capture: false,
  force_secure_context: false,
  auto_secure_context: true,
  window_opacity: 100,
  boss_key: '',
  edge_autohide: false,
  edge_autohide_edges: ['right'],
  reduce_motion: false,
  language: 'zh',
  vrchat_osc_enabled: false,
  vrchat_osc_host: '127.0.0.1',
  vrchat_osc_port: 9000,
  vrchat_osc_mode: 'clock',
  vrchat_osc_custom_text: '',
};

export const api = {
  bootstrap: () => invoke<Bootstrap>('get_bootstrap'),
  saveSettings: (settings: Settings) => invoke<Settings>('save_settings', { settings }),
  setWindowOpacity: (opacity: number) => invoke<void>('set_window_opacity', { opacity }),
  goHome: () => invoke<LayoutState>('go_home'),
  openRemote: (address: string, remember: boolean) =>
    invoke<LayoutState>('open_remote', { address, remember }),
  refreshRemote: () => invoke<LayoutState>('refresh_remote'),
  captureRemoteBackdrop: () => invoke<string | null>('capture_remote_backdrop'),
  setOverlay: (active: boolean) => invoke<LayoutState>('set_chrome_overlay', { active }),
  quitApp: () => invoke<void>('quit_app'),
};
