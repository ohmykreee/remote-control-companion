<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as Select from '$lib/components/ui/select/index.js';
  import { Progress } from '$lib/components/ui/progress/index.js';
  import { companion } from '$lib/state.svelte';
  import { copy } from '$lib/copy';

  let devices = $state<MediaDeviceInfo[]>([]);
  let selectedLabel = $state(companion.settings.microphone_label);
  let level = $state(0);
  let error = $state('');
  let previewing = $state(false);
  let preparing = $state(true);

  let stream: MediaStream | null = null;
  let audioContext: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let frame = 0;
  let disposed = false;

  const selectedValue = $derived(selectedLabel || '__default__');
  const hasSavedFallback = $derived(
    !!selectedLabel && !devices.some((device) => device.label === selectedLabel),
  );

  async function listDevices(): Promise<MediaDeviceInfo[]> {
    if (!navigator.mediaDevices?.enumerateDevices) {
      throw new Error(copy.settings.microphone.unavailable);
    }
    return (await navigator.mediaDevices.enumerateDevices()).filter(
      (device) => device.kind === 'audioinput',
    );
  }

  function stopPreview(): void {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    previewing = false;
    level = 0;
    stream?.getTracks().forEach((track) => track.stop());
    stream = null;
    analyser = null;
    if (audioContext) {
      void audioContext.close().catch(() => undefined);
      audioContext = null;
    }
  }

  function trackLevel(): void {
    if (!analyser) return;
    const samples = new Uint8Array(analyser.fftSize);
    let smoothed = 0;

    const tick = (): void => {
      if (disposed || !analyser) return;
      analyser.getByteTimeDomainData(samples);
      let sum = 0;
      for (const sample of samples) {
        const normalized = (sample - 128) / 128;
        sum += normalized * normalized;
      }
      const rms = Math.sqrt(sum / samples.length);
      const next = Math.min(100, Math.round(rms * 360));
      smoothed = Math.max(next, smoothed * 0.78);
      level = Math.round(smoothed);
      frame = requestAnimationFrame(tick);
    };

    tick();
  }

  async function startPreview(deviceId?: string): Promise<void> {
    stopPreview();
    error = '';
    if (!navigator.mediaDevices?.getUserMedia) {
      error = copy.settings.microphone.unavailable;
      return;
    }
    try {
      const constraints: MediaStreamConstraints = {
        audio: deviceId
          ? { deviceId: { exact: deviceId }, echoCancellation: false, noiseSuppression: false }
          : { echoCancellation: false, noiseSuppression: false },
        video: false,
      };
      const next = await navigator.mediaDevices.getUserMedia(constraints);
      if (disposed) {
        next.getTracks().forEach((track) => track.stop());
        return;
      }
      stream = next;
      audioContext = new AudioContext();
      await audioContext.resume().catch(() => undefined);
      const source = audioContext.createMediaStreamSource(next);
      analyser = audioContext.createAnalyser();
      analyser.fftSize = 512;
      analyser.smoothingTimeConstant = 0.78;
      source.connect(analyser);
      previewing = true;
      trackLevel();
    } catch {
      previewing = false;
      error = copy.settings.microphone.fallback;
    }
  }

  async function init(): Promise<void> {
    preparing = true;
    try {
      let found = await listDevices();
      if (!found.some((device) => device.label)) {
        const warm = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
        warm.getTracks().forEach((track) => track.stop());
        found = await listDevices();
      }
      if (disposed) return;
      devices = found;
      const target = selectedLabel
        ? found.find((device) => device.label === selectedLabel)
        : undefined;
      if (selectedLabel && !target) {
        selectedLabel = companion.settings.microphone_label;
      }
      await startPreview(target?.deviceId);
    } catch {
      error = copy.settings.microphone.denied;
    } finally {
      preparing = false;
    }
  }

  async function choose(value: string): Promise<void> {
    if (value === '__default__') {
      selectedLabel = '';
      await companion.save({ microphone_id: '', microphone_label: '' });
      await startPreview();
      return;
    }
    const device = devices.find((item) => item.label === value);
    if (!device) return;
    selectedLabel = value;
    await companion.save({ microphone_id: device.deviceId, microphone_label: device.label });
    await startPreview(device.deviceId);
  }

  onMount(() => {
    void init();
  });

  onDestroy(() => {
    disposed = true;
    stopPreview();
  });
</script>

<section class="setting-section">
  <div class="section-heading">
    <h3>{copy.settings.microphone.title}</h3>
    <p>{copy.settings.microphone.description}</p>
  </div>

  <span class="field-label">{copy.settings.microphone.label}</span>
  <Select.Root type="single" value={selectedValue} onValueChange={(value) => value && void choose(value)}>
    <Select.Trigger class="mic-trigger" aria-label={copy.settings.microphone.label}>
      <Select.Value>
        {#if !selectedLabel}
          {copy.settings.microphone.defaultDevice}
        {:else if hasSavedFallback}
          {copy.settings.microphone.fallbackOption}
        {:else}
          {selectedLabel}
        {/if}
      </Select.Value>
    </Select.Trigger>
    <Select.Content>
      <Select.Item value="__default__" label={copy.settings.microphone.defaultDevice}>
        {copy.settings.microphone.defaultDevice}
      </Select.Item>
      {#if hasSavedFallback}
        <Select.Item value={selectedLabel} label={copy.settings.microphone.fallbackOption}>
          {copy.settings.microphone.fallbackOption}
        </Select.Item>
      {/if}
      {#each devices as device (device.deviceId)}
        <Select.Item value={device.label} label={device.label}>
          {device.label}
        </Select.Item>
      {/each}
    </Select.Content>
  </Select.Root>

  <div class="level-row">
    <span class="level-status">
      {#if previewing}
        <span class="live-dot"></span>
        <span>{copy.settings.microphone.listening}</span>
      {:else}
        <span>{preparing ? '准备中' : copy.settings.microphone.idle}</span>
      {/if}
    </span>
    <Progress value={level} class="mic-progress" />
    <span class="level-value numeric">{level}%</span>
  </div>

  {#if error}
    <p class="field-error">{error}</p>
  {/if}
</section>

<style>
  .setting-section {
    padding: 16px 18px;
  }

  .section-heading {
    margin-bottom: 13px;
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

  .field-label {
    display: block;
    margin-bottom: 6px;
    color: #8d99ac;
    font-size: 11px;
  }

  :global(.mic-trigger) {
    width: 100%;
    height: 34px;
    justify-content: space-between;
    border-radius: 7px;
    border-color: rgb(255 255 255 / 8%);
    background: rgb(255 255 255 / 3%);
    padding-inline: 10px;
    color: #dce3ee;
    font-size: 11.5px;
  }

  .level-row {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 10px;
  }

  .level-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 70px;
    flex: none;
    color: #6f7d91;
    font-size: 10px;
  }

  .live-dot {
    width: 5px;
    height: 5px;
    border-radius: 99px;
    background: var(--success);
    flex: none;
  }

  :global(.mic-progress) {
    flex: 1;
    height: 3px;
    background: rgb(255 255 255 / 6%);
  }

  :global(.mic-progress [data-slot='progress-indicator']) {
    background: #6386ff;
    transition: transform 80ms linear;
  }

  .level-value {
    width: 32px;
    text-align: right;
    color: #7f8ca0;
    font-family: var(--font-mono);
    font-size: 9.5px;
    flex: none;
  }

  .field-error {
    margin: 8px 0 0;
    color: #df96a2;
    font-size: 10.5px;
    line-height: 1.5;
  }
</style>
