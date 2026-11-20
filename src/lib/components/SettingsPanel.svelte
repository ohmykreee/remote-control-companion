<script lang="ts">
  import { Switch } from '$lib/components/ui/switch/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import { Slider } from '$lib/components/ui/slider/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Checkbox } from '$lib/components/ui/checkbox/index.js';
  import MicrophonePanel from './MicrophonePanel.svelte';
  import { companion } from '$lib/state.svelte';
  import { copy } from '$lib/copy.svelte';
  import { api, type EdgeSide } from '$lib/api';

  let { dialog = false }: { dialog?: boolean } = $props();

  let opacity = $state(companion.settings.window_opacity);
  let bossRecording = $state(false);
  let bossError = $state('');
  let bossInput = $state<HTMLInputElement | null>(null);

  const edgeOptions: EdgeSide[] = ['top', 'left', 'right'];

  function edgeLabel(edge: EdgeSide): string {
    if (edge === 'top') return copy.settings.window.edgeTop;
    if (edge === 'left') return copy.settings.window.edgeLeft;
    return copy.settings.window.edgeRight;
  }

  function setReplacement(checked: boolean): void {
    void companion.save({ replace_clipboard_with_vr: checked });
  }

  function setDelay(checked: boolean): void {
    void companion.save({ delay_vr_capture: checked });
  }

  function setSecureContext(checked: boolean): void {
    void companion.save({ force_secure_context: checked });
  }

  function clampOpacity(value: number): number {
    return Math.min(100, Math.max(5, Math.round(value)));
  }

  function previewOpacity(value: number): void {
    opacity = clampOpacity(value);
    void api.setWindowOpacity(opacity).catch(() => {
      // Persisting on commit is the source of truth.
    });
  }

  function commitOpacity(value: number): void {
    opacity = clampOpacity(value);
    void companion.save({ window_opacity: opacity });
  }

  function beginBossCapture(): void {
    bossRecording = true;
    bossError = '';
  }

  function endBossCapture(): void {
    bossRecording = false;
  }

  function shortcutKey(event: KeyboardEvent): string | null {
    if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
    if (/^Digit[0-9]$/.test(event.code)) return event.code.slice(5);
    if (/^F([1-9]|1[0-2])$/.test(event.code)) return event.code;
    return null;
  }

  function recordBossKey(event: KeyboardEvent): void {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      bossRecording = false;
      bossInput?.blur();
      return;
    }
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;
    bossError = '';

    const modifiers: string[] = [];
    if (event.ctrlKey) modifiers.push('Ctrl');
    if (event.altKey) modifiers.push('Alt');
    if (event.shiftKey) modifiers.push('Shift');
    const key = shortcutKey(event);
    if (!key || modifiers.length === 0) {
      bossError = copy.settings.window.bossKeyInvalid;
      return;
    }

    const accelerator = [...modifiers, key].join('+');
    bossRecording = false;
    bossInput?.blur();
    void companion.save({ boss_key: accelerator });
  }

  function clearBossKey(): void {
    bossError = '';
    bossRecording = false;
    void companion.save({ boss_key: '' });
  }

  function setEdgeAutoHide(checked: boolean): void {
    void companion.save({ edge_autohide: checked });
  }

  function setReduceMotion(checked: boolean): void {
    void companion.save({ reduce_motion: checked });
  }

  function hasEdge(edge: EdgeSide): boolean {
    return companion.settings.edge_autohide_edges.includes(edge);
  }

  function setEdge(edge: EdgeSide, checked: boolean): void {
    const next = new Set(companion.settings.edge_autohide_edges);
    if (checked) next.add(edge);
    else next.delete(edge);
    void companion.save({
      edge_autohide_edges: edgeOptions.filter((value) => next.has(value)),
    });
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
      <h3>{copy.settings.window.title}</h3>
      <p>{copy.settings.window.description}</p>
    </div>

    <div class="field-block">
      <div class="field-head">
        <span class="field-title">{copy.settings.window.opacity}</span>
        <span class="field-value">{opacity}%</span>
      </div>
      <Slider
        type="single"
        value={opacity}
        min={5}
        max={100}
        step={1}
        aria-label={copy.settings.window.opacity}
        onValueChange={previewOpacity}
        onValueCommit={commitOpacity}
        class="opacity-slider"
      />
      <p class="field-hint">{copy.settings.window.opacityHint}</p>
    </div>

    <div class="field-block">
      <div class="field-head">
        <span class="field-title">{copy.settings.window.bossKey}</span>
      </div>
      <div class="boss-row">
        <Input
          bind:ref={bossInput}
          class="boss-input {bossRecording ? 'recording' : ''}"
          readonly
          spellcheck={false}
          autocomplete="off"
          value={bossRecording ? '' : companion.settings.boss_key}
          placeholder={bossRecording
            ? copy.settings.window.bossKeyRecording
            : copy.settings.window.bossKeyPlaceholder}
          aria-label={copy.settings.window.bossKey}
          onfocus={beginBossCapture}
          onblur={endBossCapture}
          onkeydown={recordBossKey}
          onclick={() => bossInput?.focus()}
        />
        {#if companion.settings.boss_key}
          <Button variant="ghost" size="sm" class="clear-key" onclick={clearBossKey}>
            {copy.settings.window.bossKeyClear}
          </Button>
        {/if}
      </div>
      {#if bossError}
        <p class="field-error">{bossError}</p>
      {:else}
        <p class="field-hint">{copy.settings.window.bossKeyHint}</p>
      {/if}
    </div>

    <div class="toggle-row motion-row">
      <div class="toggle-copy">
        <span class="toggle-title">{copy.settings.window.reduceMotion}</span>
        <span class="toggle-hint">{copy.settings.window.reduceMotionHint}</span>
      </div>
      <Switch
        checked={companion.settings.reduce_motion}
        onCheckedChange={setReduceMotion}
        aria-label={copy.settings.window.reduceMotion}
      />
    </div>

    <div class="toggle-row edge-row">
      <div class="toggle-copy">
        <span class="toggle-title">{copy.settings.window.edge}</span>
        <span class="toggle-hint">{copy.settings.window.edgeHint}</span>
      </div>
      <Switch
        checked={companion.settings.edge_autohide}
        onCheckedChange={setEdgeAutoHide}
        aria-label={copy.settings.window.edge}
      />
    </div>

    <div class="edge-directions">
      <div class="field-head">
        <span class="field-title">{copy.settings.window.edgeDirections}</span>
        <span class="field-hint-inline">{copy.settings.window.edgeDirectionsHint}</span>
      </div>
      <div class="edge-options">
        {#each edgeOptions as edge (edge)}
          <label class="edge-choice">
            <Checkbox
              checked={hasEdge(edge)}
              onCheckedChange={(checked) => setEdge(edge, checked === true)}
              aria-label={edgeLabel(edge)}
            />
            <span>{edgeLabel(edge)}</span>
          </label>
        {/each}
      </div>
    </div>

    <p class="section-note">{copy.settings.window.edgeNote}</p>
  </section>

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

  .field-block {
    margin-bottom: 14px;
  }

  .field-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    min-width: 0;
  }

  .field-title {
    color: #dfe6f1;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.4;
  }

  .field-value {
    color: #8ea0c4;
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .field-hint {
    margin: 5px 0 0;
    color: #748196;
    font-size: 11px;
    line-height: 1.5;
  }

  .field-hint-inline {
    color: #5e6b7f;
    font-size: 10.5px;
    text-align: right;
  }

  .field-error {
    margin: 5px 0 0;
    color: #e295a1;
    font-size: 11px;
    line-height: 1.5;
  }

  :global(.opacity-slider) {
    margin: 9px 0 2px;
  }

  .boss-row {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 6px;
  }

  :global(.boss-input) {
    height: 32px;
    flex: 1;
    border-color: rgb(255 255 255 / 9%);
    border-radius: 7px;
    background: rgb(255 255 255 / 3%);
    font-family: var(--font-mono);
    font-size: 11px;
    color: #dce5f4;
    cursor: text;
  }

  :global(.boss-input.recording) {
    border-color: rgb(111 139 255 / 55%);
    box-shadow: 0 0 0 2px rgb(82 110 245 / 12%);
  }

  :global(.boss-input::placeholder) {
    color: #505d71;
  }

  :global(.clear-key) {
    height: 30px;
    padding-inline: 8px;
    color: #748196;
    font-size: 11px;
    box-shadow: none;
  }

  :global(.clear-key:hover) {
    color: #dce5f4;
  }

  .edge-row {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid rgb(255 255 255 / 5%);
  }

  .motion-row {
    margin-top: 12px;
  }

  .edge-directions {
    margin-top: 10px;
    padding: 10px 11px;
    border: 1px solid rgb(255 255 255 / 6%);
    border-radius: 8px;
    background: rgb(255 255 255 / 1.5%);
  }

  .edge-options {
    display: flex;
    gap: 18px;
    margin-top: 9px;
  }

  .edge-choice {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: #a5b2c7;
    font-size: 11.5px;
    cursor: default;
  }
</style>
