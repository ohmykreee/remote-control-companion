<script lang="ts">
  import Minus from '@lucide/svelte/icons/minus';
  import Square from '@lucide/svelte/icons/square';
  import X from '@lucide/svelte/icons/x';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { Button } from '$lib/components/ui/button/index.js';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
  import { companion } from '$lib/state.svelte';
  import { api } from '$lib/api';
  import { copy } from '$lib/copy';

  let {
    onOpenSettings,
    onOpenAbout,
    onMenuOpened,
    onMenuClosed,
  }: {
    onOpenSettings: () => void;
    onOpenAbout: () => void;
    onMenuOpened: () => void;
    onMenuClosed: () => void;
  } = $props();

  const appWindow = getCurrentWindow();
  let menuOpen = $state(false);

  function handleMenuOpenChange(open: boolean): void {
    menuOpen = open;
    if (open) {
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

<div class="titlebar">
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
    <DropdownMenu.Content class="app-menu min-w-[164px]" sideOffset={5} align="start">
      <DropdownMenu.Item class="app-menu-item" onSelect={openSettings}>
        {copy.titlebar.settings}
      </DropdownMenu.Item>
      <DropdownMenu.Item class="app-menu-item" onSelect={openAbout}>
        {copy.titlebar.about}
      </DropdownMenu.Item>
      <DropdownMenu.Separator class="my-1" />
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
      {companion.refreshing ? '刷新中' : '刷新'}
    </Button>
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
    z-index: 30;
    -webkit-user-select: none;
    user-select: none;
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
</style>
