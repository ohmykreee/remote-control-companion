<script lang="ts">
  import Info from '@lucide/svelte/icons/info';
  import Database from '@lucide/svelte/icons/database';
  import Monitor from '@lucide/svelte/icons/monitor';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { companion } from '$lib/state.svelte';
  import { copy } from '$lib/copy';
  import appIcon from '../../../src-tauri/icons/icon.png';

  let {
    open = $bindable(false),
    onOpenChange = () => undefined,
  }: {
    open?: boolean;
    onOpenChange?: (open: boolean) => void;
  } = $props();
</script>

<Dialog.Root bind:open {onOpenChange}>
  <Dialog.Content class="about-dialog w-[calc(100vw-3rem)] max-w-[560px] gap-0 p-0 sm:max-w-[520px]">
    <div class="about-head">
      <div class="about-glow"></div>
      <img class="about-mark" src={appIcon} alt="" aria-hidden="true" />
      <div class="about-title-row">
        <div>
          <Dialog.Title class="about-title">{copy.about.title}</Dialog.Title>
          <p class="about-kicker">DSH REMOTE CONTROL COMPANION</p>
        </div>
        <span class="version-pill">v{companion.appInfo?.version ?? '0.1.0'}</span>
      </div>
      <Dialog.Description class="about-description">
        {copy.about.description}
      </Dialog.Description>
    </div>

    <div class="about-body">
      <div class="info-row">
        <span class="row-icon"><Info class="size-3.5" /></span>
        <span class="row-label">{copy.about.version}</span>
        <span class="row-value numeric">{companion.appInfo?.version ?? '0.1.0'}</span>
      </div>
      <div class="info-row">
        <span class="row-icon"><Monitor class="size-3.5" /></span>
        <span class="row-label">{copy.about.platform}</span>
        <span class="row-value">{companion.appInfo?.platform ?? 'Windows'}</span>
      </div>
      <div class="info-row path-row">
        <span class="row-icon"><Database class="size-3.5" /></span>
        <span class="row-label">{copy.about.data}</span>
        <span class="row-value path" title={companion.appInfo?.data_dir ?? ''}>
          {companion.appInfo?.data_dir ?? 'Data'}
        </span>
      </div>
    </div>

    <Dialog.Footer class="about-footer mx-0 mb-0 rounded-none border-t bg-transparent">
      <Button class="dismiss-button" onclick={() => (open = false)}>{copy.about.close}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.about-dialog) {
    border: 1px solid rgb(255 255 255 / 10%);
    background: #121822;
    box-shadow: 0 36px 110px rgb(0 0 0 / 58%);
    border-radius: 16px;
    overflow: hidden;
  }

  .about-head {
    position: relative;
    overflow: hidden;
    padding: 26px 26px 21px;
    border-bottom: 1px solid rgb(255 255 255 / 6%);
    background:
      radial-gradient(450px 220px at 40% -50%, rgb(76 108 255 / 19%), transparent 68%),
      linear-gradient(155deg, #151c28, #10161f);
  }

  .about-glow {
    position: absolute;
    width: 180px;
    height: 180px;
    right: -70px;
    top: -100px;
    border-radius: 999px;
    background: rgb(78 105 255 / 13%);
    filter: blur(26px);
  }

  .about-mark {
    position: relative;
    display: block;
    width: 46px;
    height: 46px;
    border-radius: 13px;
    object-fit: cover;
    box-shadow: 0 9px 28px rgb(38 68 210 / 27%);
    margin-bottom: 17px;
  }

  .about-title-row {
    position: relative;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  :global(.about-title) {
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.018em;
    color: #f3f6fc;
  }

  .about-kicker {
    margin: 6px 0 0;
    color: #66748d;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.12em;
  }

  .version-pill {
    flex: none;
    margin-top: 2px;
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid rgb(255 255 255 / 8%);
    background: rgb(255 255 255 / 3%);
    color: #7e8ca2;
    font-family: var(--font-mono);
    font-size: 9.5px;
  }

  :global(.about-description) {
    position: relative;
    margin: 15px 0 0;
    max-width: 430px;
    color: #8794a8;
    font-size: 12px;
    line-height: 1.7;
  }

  .about-body {
    padding: 11px 20px;
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 6px;
    min-width: 0;
  }

  .row-icon {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    flex: none;
    color: #7d8ba1;
    background: rgb(255 255 255 / 3%);
  }

  .row-label {
    color: #758297;
    font-size: 11.5px;
    flex: none;
  }

  .row-value {
    margin-left: auto;
    color: #b9c3d2;
    font-size: 11.5px;
    text-align: right;
    min-width: 0;
  }

  .row-value.path {
    font-family: var(--font-mono);
    font-size: 10px;
    color: #8d9bb0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  :global(.about-footer) {
    padding: 12px 20px 18px;
    border-top: 1px solid rgb(255 255 255 / 6%);
    display: flex;
    justify-content: flex-end;
  }

  :global(.dismiss-button) {
    min-width: 84px;
    background: rgb(255 255 255 / 5%);
    border-color: rgb(255 255 255 / 8%);
    color: #d5ddea;
  }

  :global(.dismiss-button:hover) {
    background: rgb(255 255 255 / 8%);
  }
</style>
