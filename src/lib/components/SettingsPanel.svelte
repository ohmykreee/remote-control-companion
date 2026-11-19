<script lang="ts">
  import { Switch } from '$lib/components/ui/switch/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import MicrophonePanel from './MicrophonePanel.svelte';
  import { companion } from '$lib/state.svelte';
  import { copy } from '$lib/copy';

  let { dialog = false }: { dialog?: boolean } = $props();

  function setReplacement(checked: boolean): void {
    void companion.save({ replace_clipboard_with_vr: checked });
  }

  function setDelay(checked: boolean): void {
    void companion.save({ delay_vr_capture: checked });
  }

  function setSecureContext(checked: boolean): void {
    void companion.save({ force_secure_context: checked });
  }
</script>

<div class="settings-panel" class:dialog>
  {#if !dialog}
    <header class="settings-header">
      <h2>{copy.settings.title}</h2>
      <p>{copy.settings.subtitle}</p>
    </header>
  {/if}

  <MicrophonePanel />

  <Separator />

  <section class="setting-section">
    <div class="section-heading">
      <h3>{copy.settings.screenshot.title}</h3>
      <p>{copy.settings.screenshot.description}</p>
    </div>

    <div class="toggle-row">
      <div class="toggle-copy">
        <span class="toggle-title">{copy.settings.screenshot.replace}</span>
        <span class="toggle-hint">{copy.settings.screenshot.replaceHint}</span>
      </div>
      <Switch
        checked={companion.settings.replace_clipboard_with_vr}
        onCheckedChange={setReplacement}
        aria-label={copy.settings.screenshot.replace}
      />
    </div>

    {#if companion.settings.replace_clipboard_with_vr}
      <div class="toggle-row sub">
        <div class="toggle-copy">
          <span class="toggle-title">{copy.settings.screenshot.delay}</span>
          <span class="toggle-hint">{copy.settings.screenshot.delayHint}</span>
        </div>
        <Switch
          checked={companion.settings.delay_vr_capture}
          onCheckedChange={setDelay}
          aria-label={copy.settings.screenshot.delay}
        />
      </div>
    {/if}

    <p class="section-note">{copy.settings.screenshot.note}</p>
  </section>

  <Separator />

  <section class="setting-section">
    <div class="section-heading">
      <h3>{copy.settings.security.title}</h3>
      <p>{copy.settings.security.description}</p>
    </div>

    <div class="toggle-row">
      <div class="toggle-copy">
        <span class="toggle-title">{copy.settings.security.force}</span>
        <span class="toggle-hint">{copy.settings.security.forceHint}</span>
      </div>
      <Switch
        checked={companion.settings.force_secure_context}
        onCheckedChange={setSecureContext}
        aria-label={copy.settings.security.force}
      />
    </div>

    <p class="section-note">{copy.settings.security.note}</p>
  </section>
</div>

<style>
  .settings-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .settings-panel.dialog {
    max-height: min(74vh, 700px);
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .settings-header {
    padding: 20px 18px 14px;
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 580;
    letter-spacing: -0.01em;
    color: #f0f4fb;
  }

  .settings-header p {
    margin: 4px 0 0;
    color: #748195;
    font-size: 11.5px;
    line-height: 1.55;
  }

  .setting-section {
    padding: 16px 18px;
  }

  .section-heading {
    margin-bottom: 12px;
  }

  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 560;
    color: #edf1f8;
  }

  .section-heading p {
    margin: 3px 0 0;
    color: #78859a;
    font-size: 11.5px;
    line-height: 1.5;
  }

  .toggle-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
  }

  .toggle-row.sub {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid rgb(255 255 255 / 5%);
  }

  .toggle-copy {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .toggle-title {
    color: #dfe6f1;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.4;
  }

  .toggle-hint {
    color: #748196;
    font-size: 11px;
    line-height: 1.5;
  }

  .section-note {
    margin: 9px 0 0;
    color: #5e6b7f;
    font-size: 10.5px;
    line-height: 1.5;
  }
</style>
