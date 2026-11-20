<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import AlertCircle from '@lucide/svelte/icons/alert-circle';
  import Loader2 from '@lucide/svelte/icons/loader-2';
  import appIcon from '../src-tauri/icons/icon.png';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import HomeView from '$lib/components/HomeView.svelte';
  import SettingsPanel from '$lib/components/SettingsPanel.svelte';
  import AboutDialog from '$lib/components/AboutDialog.svelte';
  import { Toaster } from '$lib/components/ui/sonner/index.js';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { api } from '$lib/api';
  import { companion } from '$lib/state.svelte';
  import { copy, setCopyLocale } from '$lib/copy.svelte';

  let settingsOpen = $state(false);
  let aboutOpen = $state(false);
  let menuOpen = $state(false);
  let remoteBackdrop = $state<string | null>(null);
  let backdropTask: Promise<void> | null = null;

  onMount(() => {
    void companion.init();
    let stopLanguageListener: (() => void) | undefined;
    void listen<string>('remote-language', (event) => {
      const language = event.payload === 'en' ? 'en' : 'zh';
      if (companion.settings.language !== language) {
        void companion.save({ language });
      }
    })
      .then((unlisten) => {
        stopLanguageListener = unlisten;
      })
      .catch(() => {
        // Keep the saved language if remote events are unavailable.
      });
    return () => stopLanguageListener?.();
  });

  $effect(() => {
    setCopyLocale(companion.settings.language);
    document.documentElement.classList.toggle(
      'reduce-motion',
      companion.settings.reduce_motion,
    );
  });

  function backToHome(): void {
    remoteBackdrop = null;
    void companion.goHome();
  }

  async function captureBackdrop(): Promise<void> {
    if (backdropTask) return backdropTask;
    backdropTask = (async () => {
      try {
        const encoded = await api.captureRemoteBackdrop();
        if (!encoded) return;
        const source = `data:image/jpeg;base64,${encoded}`;
        const image = new Image();
        const loaded = new Promise<void>((resolve) => {
          image.onload = () => resolve();
          image.onerror = () => resolve();
        });
        image.src = source;
        await loaded;
        remoteBackdrop = source;
      } catch {
        // The overlay still works with its plain dark fallback.
      } finally {
        backdropTask = null;
      }
    })();
    return backdropTask;
  }

  async function ensureOverlay(): Promise<void> {
    if (!companion.isRemote || companion.overlay) return;
    await captureBackdrop();
    if (!menuOpen && !settingsOpen && !aboutOpen) {
      remoteBackdrop = null;
      return;
    }
    await companion.ensureOverlay();
  }

  function maybeReleaseOverlay(): void {
    window.setTimeout(async () => {
      if (menuOpen || settingsOpen || aboutOpen) return;
      await companion.releaseOverlay();
      remoteBackdrop = null;
    }, 0);
  }

  function handleMenuOpened(): void {
    menuOpen = true;
    void ensureOverlay();
  }

  function handleMenuClosed(): void {
    menuOpen = false;
    maybeReleaseOverlay();
  }

  function openSettings(): void {
    settingsOpen = true;
    void ensureOverlay();
  }

  function openAbout(): void {
    aboutOpen = true;
    void ensureOverlay();
  }

  function settingsChanged(open: boolean): void {
    if (!open) maybeReleaseOverlay();
  }

  function keepTitlebarUsable(event: PointerEvent): void {
    const target = event.target;
    if (target instanceof Element && target.closest('.titlebar')) {
      event.preventDefault();
    }
  }

  function aboutChanged(open: boolean): void {
    if (!open) maybeReleaseOverlay();
  }
</script>

<div class="app-shell">
  <TitleBar
    onOpenSettings={openSettings}
    onOpenAbout={openAbout}
    onBackToHome={backToHome}
    onMenuOpened={handleMenuOpened}
    onMenuClosed={handleMenuClosed}
  />

  <div class="app-body">
    {#if !companion.ready}
      <div class="center-state">
        <img class="loading-mark" src={appIcon} alt="" aria-hidden="true" />
        <div class="loading-line">
          <Loader2 class="size-3.5 animate-spin" />
          <span>{copy.app.loading}</span>
        </div>
      </div>
    {:else if companion.fatalError}
      <div class="center-state">
        <div class="error-mark"><AlertCircle class="size-5" /></div>
        <h2>{copy.app.bootFailed}</h2>
        <p>{companion.fatalError}</p>
      </div>
    {:else if companion.view === 'home'}
      <HomeView />
    {:else if companion.overlay}
      <div class="remote-backdrop">
        {#if remoteBackdrop}
          <img class="backdrop-image" src={remoteBackdrop} alt="" aria-hidden="true" />
        {/if}
        <div class="backdrop-veil"></div>
      </div>
    {/if}
  </div>

  <Dialog.Root bind:open={settingsOpen} onOpenChange={settingsChanged}>
    <Dialog.Content
      class="settings-dialog max-w-[calc(100vw-2rem)] gap-0 p-0 sm:max-w-[640px]"
      onInteractOutside={keepTitlebarUsable}
    >
      <Dialog.Header class="settings-dialog-head">
        <Dialog.Title class="dialog-title">{copy.settings.title}</Dialog.Title>
        <Dialog.Description class="dialog-description">{copy.settings.subtitle}</Dialog.Description>
      </Dialog.Header>
      <div class="settings-dialog-body">
        <SettingsPanel dialog />
      </div>
    </Dialog.Content>
  </Dialog.Root>

  <AboutDialog bind:open={aboutOpen} onOpenChange={aboutChanged} />
  <Toaster theme="dark" position="bottom-right" />
</div>

<style>
  .app-shell {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--background);
    overflow: hidden;
  }

  .app-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .center-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    background:
      radial-gradient(600px 420px at 50% 44%, rgb(71 102 235 / 9%), transparent 70%),
      #0b0f16;
  }

  .loading-mark {
    width: 54px;
    height: 54px;
    border-radius: 16px;
    object-fit: cover;
    box-shadow: 0 16px 42px rgb(31 59 191 / 28%);
  }

  .loading-line {
    display: flex;
    align-items: center;
    gap: 7px;
    color: #78859a;
    font-size: 11.5px;
  }

  .error-mark {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    color: #f192a1;
    background: rgb(244 125 142 / 9%);
    border: 1px solid rgb(244 125 142 / 14%);
  }

  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 560;
    color: #edf1f8;
  }

  .center-state p {
    margin: 0;
    max-width: 360px;
    text-align: center;
    color: #7d899c;
    font-size: 12px;
    line-height: 1.7;
  }

  .remote-backdrop {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background:
      radial-gradient(900px 560px at 30% 0%, rgb(50 76 190 / 8%), transparent 65%),
      #0b0f16;
  }

  .backdrop-image {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(18px) brightness(0.78) contrast(1.04) saturate(0.92);
    transform: scale(1.08);
    transform-origin: center;
    opacity: 0.96;
  }

  .backdrop-veil {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(80% 80% at 50% 20%, transparent 0%, rgb(6 9 14 / 10%) 75%),
      linear-gradient(180deg, rgb(6 9 14 / 8%), rgb(6 9 14 / 30%));
  }

  :global(.settings-dialog) {
    display: flex;
    flex-direction: column;
    max-height: min(86vh, 800px);
    overflow: hidden;
    border: 1px solid rgb(255 255 255 / 9%);
    background: #10161f;
    box-shadow: 0 38px 120px rgb(0 0 0 / 62%);
    border-radius: 16px;
  }

  :global(.settings-dialog-head) {
    flex: none;
    padding: 20px 54px 17px 24px;
    border-bottom: 1px solid rgb(255 255 255 / 8%);
    background: #111821;
  }

  :global(.settings-dialog-body) {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    background: #0d131c;
  }

  :global(.dialog-title) {
    font-size: 17px;
    font-weight: 600;
    color: #f2f6fd;
    letter-spacing: -0.01em;
  }

  :global(.dialog-description) {
    margin-top: 5px;
    color: #7b8799;
    font-size: 11.5px;
    line-height: 1.6;
  }
</style>
