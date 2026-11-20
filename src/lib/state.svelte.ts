import { api, DEFAULT_SETTINGS, type AppInfo, type LayoutState, type Settings } from './api';
import { copy, setCopyLocale } from './copy.svelte';
import { toast } from './toast';

export type ViewMode = 'home' | 'remote';

function normalizeAddress(raw: string): string {
  const value = raw.trim();
  if (!value) throw new Error(copy.home.emptyAddress);
  const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(value) ? value : `http://${value}`;
  let parsed: URL;
  try {
    parsed = new URL(withScheme);
  } catch {
    throw new Error(copy.home.invalidAddress);
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error(copy.home.invalidAddress);
  }
  if (!parsed.hostname) throw new Error(copy.home.invalidAddress);
  return parsed.toString();
}

class CompanionApp {
  ready = $state(false);
  fatalError = $state('');
  appInfo = $state<AppInfo | null>(null);
  settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  view = $state<ViewMode>('home');
  overlay = $state(false);
  address = $state('');
  secureContextApplied = $state(false);
  connecting = $state(false);
  refreshing = $state(false);
  connectError = $state('');
  private saveQueue: Promise<void> = Promise.resolve();

  get isRemote(): boolean {
    return this.view === 'remote';
  }

  async init(): Promise<void> {
    try {
      const boot = await api.bootstrap();
      this.appInfo = boot.app;
      this.settings = boot.settings;
      setCopyLocale(boot.settings.language);
      this.address = boot.settings.remember_address ? boot.settings.remote_url : '';
      this.view = boot.layout.mode;
      this.overlay = boot.layout.overlay;
      this.secureContextApplied = boot.layout.secure_context_applied;
      this.ready = true;
    } catch {
      this.fatalError = copy.app.initFailed;
      this.ready = true;
    }
  }

  async save(patch: Partial<Settings>): Promise<void> {
    this.settings = { ...this.settings, ...patch };
    setCopyLocale(this.settings.language);
    const snapshot = { ...this.settings };
    this.saveQueue = this.saveQueue
      .then(async () => {
        await api.saveSettings(snapshot);
      })
      .catch(() => {
        toast(copy.settings.saveFailed, 'error');
      });
    await this.saveQueue;
  }

  async connect(): Promise<void> {
    if (this.connecting) return;
    this.connectError = '';
    let normalized: string;
    try {
      normalized = normalizeAddress(this.address);
    } catch (error) {
      this.connectError = error instanceof Error ? error.message : copy.home.invalidAddress;
      return;
    }

    this.connecting = true;
    try {
      await this.save({
        remote_url: normalized,
        remember_address: this.settings.remember_address,
      });
      const layout = await api.openRemote(normalized, this.settings.remember_address);
      this.applyLayout(layout);
      this.address = normalized;
      this.view = 'remote';
      this.overlay = false;
    } catch (error) {
      this.connectError =
        error instanceof Error ? error.message : copy.home.connectFailed;
    } finally {
      this.connecting = false;
    }
  }

  async goHome(): Promise<void> {
    try {
      const layout = await api.goHome();
      this.applyLayout(layout);
      this.overlay = false;
    } catch {
      toast(copy.remote.goHomeFailed, 'error');
    }
  }

  async refresh(): Promise<void> {
    if (this.refreshing) return;
    this.refreshing = true;
    try {
      const layout = await api.refreshRemote();
      this.applyLayout(layout);
    } catch {
      toast(copy.remote.refreshFailed, 'error');
    } finally {
      this.refreshing = false;
    }
  }

  async ensureOverlay(): Promise<void> {
    if (!this.isRemote || this.overlay) return;
    try {
      const layout = await api.setOverlay(true);
      this.overlay = true;
      this.applyLayout(layout);
    } catch {
      // The menu can still open over the home view; remote overlay failure is non-fatal.
    }
  }

  async releaseOverlay(): Promise<void> {
    if (!this.isRemote || !this.overlay) return;
    try {
      const layout = await api.setOverlay(false);
      this.overlay = false;
      this.applyLayout(layout);
    } catch {
      // Keep the local overlay state; the next refresh restores the layout.
    }
  }

  applyLayout(layout: LayoutState): void {
    this.view = layout.mode;
    this.overlay = layout.overlay;
    this.secureContextApplied = layout.secure_context_applied;
    if (layout.address) this.address = layout.address;
  }

  setRemember(remember: boolean): void {
    void this.save({ remember_address: remember });
  }
}

export const companion = new CompanionApp();
