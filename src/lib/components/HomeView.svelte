<script lang="ts">
  import { onMount } from 'svelte';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Checkbox } from '$lib/components/ui/checkbox/index.js';
  import SettingsPanel from './SettingsPanel.svelte';
  import { companion } from '$lib/state.svelte';
  import { copy } from '$lib/copy.svelte';

  let addressInput = $state<HTMLInputElement | null>(null);

  function submit(event: SubmitEvent): void {
    event.preventDefault();
    void companion.connect();
  }

  function setRemember(checked: boolean): void {
    companion.setRemember(checked);
  }

  onMount(() => {
    if (!companion.address) {
      queueMicrotask(() => addressInput?.focus());
    }
  });
</script>

<main class="home">
  <section class="connect-pane">
    <div class="connect-content">
      <h1>{copy.home.connectTitle}</h1>
      <p class="lead">{copy.home.subtitle}</p>

      <form class="connect-form" onsubmit={submit}>
        <label class="field-label" for="remote-address">{copy.home.addressLabel}</label>
        <div class="address-row">
          <Input
            id="remote-address"
            bind:ref={addressInput}
            bind:value={companion.address}
            placeholder={copy.home.addressPlaceholder}
            spellcheck={false}
            autocomplete="off"
            autocapitalize="off"
            aria-invalid={!!companion.connectError}
            onkeydown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                void companion.connect();
              }
            }}
          />
          <Button type="submit" class="connect-button" disabled={companion.connecting}>
            {companion.connecting ? copy.home.connecting : copy.home.connect}
          </Button>
        </div>

        {#if companion.connectError}
          <p class="form-error">{companion.connectError}</p>
        {/if}

        <label class="remember">
          <Checkbox
            checked={companion.settings.remember_address}
            onCheckedChange={(checked) => setRemember(checked === true)}
            aria-label={copy.home.remember}
          />
          <span>{copy.home.remember}</span>
        </label>
      </form>
    </div>

    <div class="connect-foot">
      <span>{copy.app.name}</span>
      <span class="foot-sep">·</span>
      <span>v{companion.appInfo?.version ?? '0.1.0'}</span>
    </div>
  </section>

  <aside class="settings-pane">
    <SettingsPanel />
  </aside>
</main>

<style>
  .home {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(280px, 0.78fr) minmax(430px, 1.22fr);
    overflow: hidden;
    background: #0b0f16;
  }

  .connect-pane {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 28px;
    background:
      radial-gradient(560px 300px at 10% -20%, rgb(58 84 190 / 9%), transparent 68%),
      #0b0f16;
    border-right: 1px solid rgb(255 255 255 / 6%);
  }

  .connect-content {
    width: 100%;
    max-width: 350px;
    margin: auto;
  }

  h1 {
    margin: 0;
    color: #f2f5fb;
    font-size: 22px;
    font-weight: 590;
    letter-spacing: -0.025em;
  }

  .lead {
    margin: 8px 0 24px;
    color: #7d899b;
    font-size: 12px;
    line-height: 1.65;
  }

  .connect-form {
    display: flex;
    flex-direction: column;
  }

  .field-label {
    margin-bottom: 7px;
    color: #98a4b6;
    font-size: 11.5px;
  }

  .address-row {
    display: flex;
    gap: 7px;
  }

  .address-row :global(input) {
    height: 36px;
    flex: 1;
    border-color: rgb(255 255 255 / 9%);
    border-radius: 8px;
    background: rgb(255 255 255 / 3%);
    font-family: var(--font-mono);
    font-size: 12px;
    color: #e5ebf5;
  }

  .address-row :global(input::placeholder) {
    color: #505d71;
  }

  :global(.connect-button) {
    height: 36px;
    padding-inline: 14px;
    border-radius: 8px;
    background: #4767ef;
    color: #fff;
    font-size: 12px;
    font-weight: 500;
    flex: none;
  }

  :global(.connect-button:hover) {
    background: #5575fa;
  }

  .form-error {
    margin: 8px 0 0;
    color: #e295a1;
    font-size: 11px;
    line-height: 1.5;
  }

  .remember {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-top: 13px;
    color: #9aa6b8;
    font-size: 11.5px;
    cursor: default;
  }

  .connect-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #4d596b;
    font-size: 10px;
  }

  .foot-sep {
    color: #394455;
  }

  .settings-pane {
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    background: #0d121a;
  }

  @media (max-width: 720px) {
    .home {
      grid-template-columns: 1fr;
      overflow-y: auto;
    }

    .connect-pane {
      min-height: 300px;
      border-right: 0;
      border-bottom: 1px solid rgb(255 255 255 / 6%);
    }

    .settings-pane {
      overflow: visible;
    }
  }
</style>
