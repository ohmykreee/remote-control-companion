import { invoke } from '@tauri-apps/api/core';

export interface Settings {
  version: number;
  remote_url: string;
  remember_address: boolean;
  microphone_id: string;
  microphone_label: string;
  replace_clipboard_with_vr: boolean;
  delay_vr_capture: boolean;
  force_secure_context: boolean;
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
}

export interface Bootstrap {
  app: AppInfo;
  settings: Settings;
  layout: LayoutState;
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
};

export const api = {
  bootstrap: () => invoke<Bootstrap>('get_bootstrap'),
  saveSettings: (settings: Settings) => invoke<Settings>('save_settings', { settings }),
  openRemote: (address: string, remember: boolean) =>
    invoke<LayoutState>('open_remote', { address, remember }),
  refreshRemote: () => invoke<LayoutState>('refresh_remote'),
  setOverlay: (active: boolean) => invoke<LayoutState>('set_chrome_overlay', { active }),
  quitApp: () => invoke<void>('quit_app'),
};
