<script lang="ts">
  import { onMount } from 'svelte';
  import Minus from '@lucide/svelte/icons/minus';
  import Square from '@lucide/svelte/icons/square';
  import X from '@lucide/svelte/icons/x';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Check from '@lucide/svelte/icons/check';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { Button } from '$lib/components/ui/button/index.js';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
  import { companion } from '$lib/state.svelte';
  import { api } from '$lib/api';
  import { copy } from '$lib/copy.svelte';
  import { toast } from '$lib/toast';

  let {
    onOpenSettings,
    onOpenAbout,
    onBackToHome,
    onMenuOpened,
    onMenuClosed,
  }: {
    onOpenSettings: () => void;
    onOpenAbout: () => void;
    onBackToHome: () => void;
    onMenuOpened: () => void;
    onMenuClosed: () => void;
  } = $props();

  const appWindow = getCurrentWindow();
  let menuOpen = $state(false);
  let alwaysOnTop = $state(false);
  let remoteTheme = $state<'light' | 'dark'>('dark');
  const light = $derived(companion.isRemote && remoteTheme === 'light');

  onMount(() => {
    void refreshAlwaysOnTop();
    let stopThemeListener: (() => void) | undefined;
    void listen<string>('remote-theme', (event) => {
      remoteTheme = event.payload === 'light' ? 'light' : 'dark';
    })
      .then((unlisten) => {
        stopThemeListener = unlisten;
      })
      .catch(() => {
        // The titlebar keeps its dark fallback if events are unavailable.
      });
    return () => stopThemeListener?.();
  });

  async function refreshAlwaysOnTop(): Promise<void> {
    try {
      alwaysOnTop = await appWindow.isAlwaysOnTop();
    } catch {
      // Keep the last known state if the platform cannot answer.
    }
  }

  async function toggleAlwaysOnTop(): Promise<void> {
    const next = !alwaysOnTop;
    try {
      await appWindow.setAlwaysOnTop(next);
      alwaysOnTop = next;
    } catch {
      toast(copy.titlebar.alwaysOnTopFailed, 'error');
    }
  }

  function handleMenuOpenChange(open: boolean): void {
    menuOpen = open;
    if (open) {
      void refreshAlwaysOnTop();
      onMenuOpened();
    } else {
      window.setTimeout(onMenuClosed, 0);
    }
  }

  function openSettings(): void {
    onOpenSettings();
    menuOpen = false;
    window.setTimeout(onMenuClosed, 0);
  }

  function openAbout(): void {
    onOpenAbout();
    menuOpen = false;
    window.setTimeout(onMenuClosed, 0);
  }

  function backToHome(): void {
    onBackToHome();
    menuOpen = false;
    window.setTimeout(onMenuClosed, 0);
  }

  function chooseLanguage(language: 'zh' | 'en'): void {
    if (companion.settings.language !== language) {
      void companion.save({ language });
    }
  }

  function quit(): void {
    menuOpen = false;
    void api.quitApp();
  }

  function refresh(): void {
    void companion.refresh();
  }

  function minimize(): void {
    void appWindow.minimize();
  }

  function maximize(): void {
    void appWindow.toggleMaximize();
  }

  function close(): void {
    void appWindow.close();
  }
</script>

<div class="titlebar" class:light>
  <div class="titlebar-drag" data-tauri-drag-region aria-hidden="true"></div>
  <DropdownMenu.Root bind:open={menuOpen} onOpenChange={handleMenuOpenChange}>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="ghost"
          size="sm"
          class="title-menu-button {menuOpen ? 'is-open' : ''}"
        >
          {copy.app.menuLabel}
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content
      class="app-menu min-w-[164px] {light ? 'app-menu-light' : ''}"
      sideOffset={5}
      align="start"
    >
      <DropdownMenu.Item
        class="app-menu-item"
        onSelect={() => void toggleAlwaysOnTop()}
      >
        {alwaysOnTop ? copy.titlebar.cancelAlwaysOnTop : copy.titlebar.alwaysOnTop}
      </DropdownMenu.Item>
      <DropdownMenu.Separator class="app-menu-separator my-1" />
      <DropdownMenu.Item class="app-menu-item" onSelect={openSettings}>
        {copy.titlebar.settings}
      </DropdownMenu.Item>
      {#if companion.isRemote}
        <DropdownMenu.Item class="app-menu-item" onSelect={backToHome}>
          {copy.titlebar.backToHome}
        </DropdownMenu.Item>
      {/if}
      <DropdownMenu.Item class="app-menu-item" onSelect={openAbout}>
        {copy.titlebar.about}
      </DropdownMenu.Item>
      <DropdownMenu.Separator class="app-menu-separator my-1" />
      <DropdownMenu.Item class="app-menu-item" onSelect={quit}>
        {copy.titlebar.quit}
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  {#if companion.isRemote}
    <Button
      variant="ghost"
      size="sm"
      class="title-menu-button"
      aria-label={copy.titlebar.refresh}
      aria-busy={companion.refreshing}
      disabled={companion.refreshing}
      onclick={refresh}
    >
      {companion.refreshing ? copy.titlebar.refreshing : copy.titlebar.refreshShort}
    </Button>
  {:else if companion.ready}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            variant="ghost"
            size="sm"
            class="title-menu-button language-button"
            aria-label={copy.language.switchTo}
            title={copy.language.label}
          >
            {companion.settings.language === 'en' ? 'English' : '中文'}
            <ChevronDown class="size-3 opacity-70" />
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        class="app-menu language-menu"
        sideOffset={5}
        align="start"
      >
        <DropdownMenu.Item
          class="app-menu-item language-item"
          onSelect={() => chooseLanguage('zh')}
        >
          <span>中文</span>
          {#if companion.settings.language === 'zh'}<Check class="size-3.5" />{/if}
        </DropdownMenu.Item>
        <DropdownMenu.Item
          class="app-menu-item language-item"
          onSelect={() => chooseLanguage('en')}
        >
          <span>English</span>
          {#if companion.settings.language === 'en'}<Check class="size-3.5" />{/if}
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  {/if}

  <div class="titlebar-spacer"></div>

  <div class="window-controls" data-no-drag>
    <Button
      variant="ghost"
      size="icon-sm"
      class="window-button"
      aria-label={copy.titlebar.minimize}
      title={copy.titlebar.minimize}
      onclick={minimize}
    >
      <Minus class="size-3.5" />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      class="window-button"
      aria-label={copy.titlebar.maximize}
      title={copy.titlebar.maximize}
      onclick={maximize}
    >
      <Square class="size-3" />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      class="window-button window-close"
      aria-label={copy.titlebar.close}
      title={copy.titlebar.close}
      onclick={close}
    >
      <X class="size-3.5" />
    </Button>
  </div>
</div>

<style>
  .titlebar {
    height: var(--titlebar-height);
    flex: 0 0 var(--titlebar-height);
    display: flex;
    align-items: center;
    gap: 1px;
    padding-left: 8px;
    background: #0b0f16;
    border-bottom: 1px solid rgb(255 255 255 / 6%);
    position: relative;
    z-index: 60;
    pointer-events: auto;
    -webkit-user-select: none;
    user-select: none;
    transition:
      background-color 160ms ease,
      border-color 160ms ease;
  }

  .titlebar-drag {
    position: absolute;
    inset: 0;
    z-index: 0;
  }

  .titlebar :global(button) {
    position: relative;
    z-index: 1;
  }

  :global(.title-menu-button) {
    height: 30px;
    padding-inline: 9px;
    border-radius: 6px;
    color: #b8c1d0;
    font-size: 12.5px;
    font-weight: 450;
    box-shadow: none;
  }

  :global(.title-menu-button:hover),
  :global(.title-menu-button.is-open) {
    background: rgb(255 255 255 / 6%);
    color: #f3f6fc;
  }

  :global(.title-menu-button:disabled) {
    opacity: 0.55;
  }

  .titlebar-spacer {
    flex: 1;
    height: 100%;
  }

  .window-controls {
    display: flex;
    align-items: stretch;
    height: 100%;
    position: relative;
    z-index: 1;
  }

  :global(.window-button) {
    width: 46px;
    height: 100%;
    border-radius: 0;
    padding: 0;
    color: #8792a4;
    box-shadow: none;
  }

  :global(.window-button:hover) {
    background: rgb(255 255 255 / 7%);
    color: #eef2f8;
  }

  :global(.window-close:hover) {
    background: #c42b3a;
    color: #fff;
  }

  :global(.app-menu) {
    border: 1px solid rgb(255 255 255 / 9%);
    background: #171d28;
    box-shadow: 0 16px 48px rgb(0 0 0 / 46%);
    min-width: 160px;
    padding: 5px;
    border-radius: 8px;
  }

  :global(.app-menu-item) {
    padding: 7px 9px;
    border-radius: 5px;
    color: #d7deea;
    font-size: 12.5px;
    cursor: default;
  }

  :global(.app-menu-item:focus) {
    background: rgb(95 122 255 / 13%);
    color: #f3f6ff;
  }

  .titlebar.light {
    background: #eef2f8;
    border-bottom-color: rgb(15 23 42 / 9%);
  }

  :global(.titlebar.light .title-menu-button) {
    color: #4f5d72;
  }

  :global(.titlebar.light .title-menu-button:hover),
  :global(.titlebar.light .title-menu-button.is-open) {
    background: rgb(15 23 42 / 6%);
    color: #111827;
  }

  :global(.titlebar.light .window-button) {
    color: #5d6a7d;
  }

  :global(.titlebar.light .window-button:hover) {
    background: rgb(15 23 42 / 6%);
    color: #111827;
  }

  :global(.titlebar.light .window-close:hover) {
    background: #c42b3a;
    color: #fff;
  }

  :global(.app-menu-light) {
    border-color: rgb(15 23 42 / 10%);
    background: #ffffff;
    box-shadow: 0 16px 48px rgb(15 23 42 / 16%);
  }

  :global(.app-menu-light .app-menu-item) {
    color: #334155;
  }

  :global(.app-menu-light .app-menu-item:focus) {
    background: #eef2f8;
    color: #0f172a;
  }

  :global(.app-menu-light .app-menu-separator) {
    background: rgb(15 23 42 / 10%);
  }

  :global(.language-button) {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  :global(.language-menu) {
    min-width: 132px;
  }

  :global(.language-item) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
</style>
