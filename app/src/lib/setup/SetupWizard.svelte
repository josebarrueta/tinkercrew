<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import {
    SetupSession,
    setupSteps,
    type SetupProgress,
    type UsbDevice,
  } from "./session";
  import { connectionError } from "../board/connection";
  import ObservedSetup, { type Observation } from "./ObservedSetup.svelte";

  let {
    onUseProvisioned,
    beforeChange,
    onProofRefresh,
  }: {
    onUseProvisioned: () => void;
    beforeChange: () => Promise<void>;
    onProofRefresh: (boardId: string | null) => Promise<void>;
  } = $props();
  const native = isTauri();
  let session: SetupSession;
  let progress = $state<SetupProgress | null>(null);
  let observation = $state<Observation | null>(null);
  let devices = $state<UsbDevice[]>([]);
  let chosenSerial = $state("");
  let confirmed = $state(false);
  let resetConfirmed = $state(false);
  let busy = $state(false);
  let error = $state("");
  let status = $state("Opening local setup checkpoints…");
  let revision = 0;
  const step = $derived(progress ? new SetupSession(progress).step : null);
  async function reloadNative() {
    if (!native) return;
    const saved = await invoke<{
      payload: string;
      revision: number;
      observation?: Observation | null;
    } | null>("load_setup");
    session = new SetupSession(saved ? JSON.parse(saved.payload) : undefined);
    revision = saved?.revision ?? 0;
    observation = saved?.observation ?? null;
    progress = session.getProgress();
    chosenSerial = progress.selectedSerial ?? "";
    await onProofRefresh(observation?.boardId ?? null);
    status =
      "Reloaded native evidence. Historical checkpoints do not grant live arming or replace physical acceptance.";
  }

  onMount(() => {
    let disposed = false;
    void (async () => {
      try {
        const saved = native
          ? await invoke<{
              payload: string;
              revision: number;
              observation?: Observation | null;
            } | null>("load_setup")
          : null;
        if (disposed) return;
        session = new SetupSession(
          saved ? JSON.parse(saved.payload) : undefined,
        );
        revision = saved?.revision ?? 0;
        observation = saved?.observation ?? null;
        progress = session.getProgress();
        chosenSerial = progress.selectedSerial ?? "";
        if (native) await onProofRefresh(observation?.boardId ?? null);
        status = native
          ? "Setup checkpoints stay in macOS Keychain. Fresh USB checks are required after reopening."
          : "Browser preview only: real setup requires the native macOS app. No Board setup success can be recorded here.";
      } catch (cause) {
        error =
          typeof cause === "string"
            ? cause
            : "Could not resume setup. Unlock Keychain and retry; existing checkpoints have not been replaced.";
      }
    })();
    return () => {
      disposed = true;
    };
  });

  async function refreshUsb() {
    devices = [];
    session.observeUsb([]);
    devices = await invoke<UsbDevice[]>("inspect_setup_usb");
    session.observeUsb(devices);
  }

  async function act(action: () => void | Promise<void>) {
    if (!native || !progress || busy) return;
    busy = true;
    error = "";
    try {
      await action();
      progress = session.getProgress();
      revision = await invoke("save_setup", {
        payload: JSON.stringify(progress),
        expectedRevision: revision,
      });
      status =
        "Setup checkpoints saved in Keychain. Human handoffs are recorded as human confirmations, not automatic verification.";
    } catch (cause) {
      confirmed = false;
      error =
        typeof cause === "string"
          ? cause
          : cause instanceof Error
            ? cause.message
            : connectionError(cause).message;
      status =
        "Check the message above; retry saving if a checkpoint was not saved. No Linux configuration was changed by this guide.";
    } finally {
      busy = false;
    }
  }

  async function officialSetup() {
    try {
      if (native) await invoke("open_official_setup");
      else
        window.open(
          "https://docs.arduino.cc/software/app-lab/configure/config/",
          "_blank",
          "noopener,noreferrer",
        );
    } catch {
      error =
        "Open https://docs.arduino.cc/software/app-lab/configure/config/ in your browser for official Arduino setup.";
    }
  }
</script>

<section class="setup" aria-label="Resumable UNO Q setup guide">
  <h3>Resumable parent setup guide</h3>
  <p>
    This is a hybrid setup guide: official tooling handles first account/OS
    setup and the explicitly approved first app/sketch start. Separate native
    actions register restricted SSH/gameplay keys, verify or update only the
    compatible Linux runtime, and request/observe a restart. A checkbox never
    proves those operations. No OS flashing or automatic firmware update runs
    here; unrelated configuration and Mission progress are preserved.
  </p>
  <p role="status">{status}</p>
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  {#if progress && step}
    <p>
      <strong>Selected identity:</strong>
      {progress.selectedSerial ?? "none — choose explicitly"}
    </p>
    <fieldset disabled={!native || busy}>
      <legend>USB identity and recovery</legend>
      <button onclick={() => act(refreshUsb)}>Refresh USB devices</button>
      <label
        >USB device serial
        <select bind:value={chosenSerial}>
          <option value="">Choose the physically connected UNO Q</option>
          {#each devices as device}<option value={device.serial}
              >{device.serial} — {device.state}</option
            >{/each}
          {#if chosenSerial && !devices.some((device) => device.serial === chosenSerial)}<option
              value={chosenSerial}
              >{chosenSerial} — saved, refresh to verify</option
            >{/if}
        </select>
      </label>
      <button
        disabled={!chosenSerial}
        onclick={() => act(() => session.selectBoard(chosenSerial))}
        >Use selected Board</button
      >
      <label class="check"
        ><input type="checkbox" bind:checked={resetConfirmed} />Start a
        different setup: clear only this guide's checkpoints, never Board
        configuration or keys</label
      >
      <button
        disabled={!resetConfirmed || !chosenSerial}
        onclick={() =>
          act(() => {
            session.startNew(chosenSerial);
            confirmed = false;
            resetConfirmed = false;
          })}>Start new setup for this serial</button
      >
    </fieldset>
    <article>
      <p>
        {progress.stageIndex === setupSteps.length
          ? "Setup recorded"
          : `Checkpoint ${progress.stageIndex + 1} of ${setupSteps.length}`}
      </p>
      <h4>{step.title}</h4>
      <p>{step.instruction}</p>
      {#if step.kind === "native" || step.kind === "complete"}
        <ObservedSetup
          stepId={step.id}
          serial={progress.selectedSerial ?? ""}
          {observation}
          {beforeChange}
          onRefresh={reloadNative}
          onBegin={onUseProvisioned}
        />
        {#if step.kind !== "complete"}<button onclick={onUseProvisioned}
            >Return to already-provisioned Board connections</button
          >
          <p>This separate connection path does not complete the guide.</p>{/if}
      {:else}
        <fieldset disabled={!native || busy || !progress.selectedSerial}>
          {#if step.kind !== "usb"}
            <label class="check"
              ><input type="checkbox" bind:checked={confirmed} />I completed
              this step for the selected UNO Q in person / in official Arduino
              tooling (or used the explicitly described supported handoff)</label
            >
          {/if}
          <button
            disabled={step.kind !== "usb" && !confirmed}
            onclick={() =>
              act(async () => {
                await refreshUsb();
                if (step?.kind !== "usb") session.confirmHumanStep();
                session.advance();
                confirmed = false;
              })}>Record checkpoint and continue →</button
          >
        </fieldset>
      {/if}
      <button onclick={() => officialSetup()}
        >Open official Arduino setup guide</button
      >
      <p>
        Wrong Wi-Fi password? Correct it in official App Lab and resume this
        same checkpoint. USB/power lost? Reconnect the same serial, wait for
        boot, and refresh. Never bypass trust checks or guess a Linux password.
      </p>
    </article>
    <details>
      <summary>All checkpoints and evidence</summary>
      <ol>
        {#each setupSteps as item, index}
          <li>
            {item.title}: {index < progress.stageIndex
              ? `recorded (${progress.checkpoints[index].source})`
              : item.kind === "native"
                ? "requires native proof"
                : "not yet recorded"}
          </li>
        {/each}
      </ol>
    </details>
    <button
      disabled={!native || busy || progress.stageIndex > 7}
      onclick={() => act(() => {})}>Retry saving checkpoints</button
    >
  {/if}
</section>

<style>
  .setup {
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px;
    margin: 16px 0;
    background: var(--surface);
  }
  h3 {
    font-size: 24px;
    margin-top: 0;
  }
  h4 {
    font-size: 20px;
  }
  fieldset {
    margin: 15px 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
  }
  label {
    display: block;
    margin: 12px 0;
  }
  select {
    display: block;
    width: 100%;
    font: inherit;
    padding: 10px;
    margin-top: 6px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  button {
    padding: 11px;
    margin: 6px 10px 6px 0;
    font: inherit;
    color: var(--primary);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .error {
    padding: 12px;
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: 8px;
  }
  summary {
    cursor: pointer;
  }
  li {
    margin: 8px 0;
  }
</style>
