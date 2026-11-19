<script lang="ts">
  import { onMount } from 'svelte';
  import ScanEye from '@lucide/svelte/icons/scan-eye';
  import AlertCircle from '@lucide/svelte/icons/alert-circle';
  import Loader2 from '@lucide/svelte/icons/loader-2';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import HomeView from '$lib/components/HomeView.svelte';
  import SettingsPanel from '$lib/components/SettingsPanel.svelte';
  import AboutDialog from '$lib/components/AboutDialog.svelte';
  import { Toaster } from '$lib/components/ui/sonner/index.js';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { companion } from '$lib/state.svelte';
  import { copy } from '$lib/copy';

  let settingsOpen = $state(false);
  let aboutOpen = $state(false);
  let menuOpen = $state(false);

  onMount(() => {
    void companion.init();
  });

  function ensureOverlay(): void {
    if (companion.isRemote && !companion.overlay) {
      void companion.ensureOverlay();
    }
  }

  function maybeReleaseOverlay(): void {
    window.setTimeout(() => {
      if (menuOpen || settingsOpen || aboutOpen) return;
      void companion.releaseOverlay();
    }, 0);
  }

  function handleMenuOpened(): void {
    menuOpen = true;
    ensureOverlay();
  }

  function handleMenuClosed(): void {
    menuOpen = false;
    maybeReleaseOverlay();
  }

  function openSettings(): void {
    settingsOpen = true;
    ensureOverlay();
  }

  function openAbout(): void {
    aboutOpen = true;
    ensureOverlay();
  }

  function settingsChanged(open: boolean): void {
    if (!open) maybeReleaseOverlay();
  }

  function aboutChanged(open: boolean): void {
    if (!open) maybeReleaseOverlay();
  }
</script>

<div class="app-shell">
  <TitleBar
    onOpenSettings={openSettings}
    onOpenAbout={openAbout}
    onMenuOpened={handleMenuOpened}
    onMenuClosed={handleMenuClosed}
  />

  <div class="app-body">
    {#if !companion.ready}
      <div class="center-state">
        <div class="loading-mark"><ScanEye class="size-6" /></div>
        <div class="loading-line">
          <Loader2 class="size-3.5 animate-spin" />
          <span>正在准备窗口</span>
        </div>
      </div>
    {:else if companion.fatalError}
      <div class="center-state">
        <div class="error-mark"><AlertCircle class="size-5" /></div>
        <h2>窗口未能启动</h2>
        <p>{companion.fatalError}</p>
      </div>
    {:else if companion.view === 'home'}
      <HomeView />
    {:else if companion.overlay}
      <div class="remote-backdrop"></div>
    {/if}
  </div>

  <Dialog.Root bind:open={settingsOpen} onOpenChange={settingsChanged}>
    <Dialog.Content class="settings-dialog max-w-[calc(100vw-2rem)] gap-0 p-0 sm:max-w-[640px]">
      <Dialog.Header class="settings-dialog-head">
        <Dialog.Title class="dialog-title">设置</Dialog.Title>
        <Dialog.Description class="dialog-description">{copy.settings.subtitle}</Dialog.Description>
      </Dialog.Header>
      <SettingsPanel dialog />
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
    display: grid;
    place-items: center;
    width: 54px;
    height: 54px;
    border-radius: 16px;
    color: #eef2ff;
    background: linear-gradient(145deg, #5878ff, #2949ca);
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
    flex: 1;
    min-height: 0;
    background:
      radial-gradient(900px 560px at 30% 0%, rgb(50 76 190 / 8%), transparent 65%),
      #0b0f16;
  }

  :global(.settings-dialog) {
    max-height: min(86vh, 840px);
    overflow: hidden;
    border: 1px solid rgb(255 255 255 / 9%);
    background: #10161f;
    box-shadow: 0 38px 120px rgb(0 0 0 / 62%);
    border-radius: 16px;
  }

  :global(.settings-dialog-head) {
    padding: 18px 20px 0;
  }

  :global(.dialog-title) {
    font-size: 16px;
    font-weight: 600;
    color: #f0f4fb;
  }

  :global(.dialog-description) {
    margin-top: 4px;
    color: #748195;
    font-size: 11.5px;
    line-height: 1.55;
  }
</style>
