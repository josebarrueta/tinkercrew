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

  let { onUseProvisioned }: { onUseProvisioned: () => void } = $props();
  const native = isTauri();
  let session: SetupSession;
  let progress = $state<SetupProgress | null>(null);
  let devices = $state<UsbDevice[]>([]);
  let chosenSerial = $state("");
  let confirmed = $state(false);
  let resetConfirmed = $state(false);
  let busy = $state(false);
  let error = $state("");
  let status = $state("Opening local setup checkpoints…");
  let revision = 0;
  const step = $derived(progress ? setupSteps[progress.stageIndex] : null);

  onMount(() => {
    let disposed = false;
    void (async () => {
      try {
        const saved = native
          ? await invoke<{ payload: string; revision: number } | null>(
              "load_setup",
            )
          : null;
        if (disposed) return;
        session = new SetupSession(
          saved ? JSON.parse(saved.payload) : undefined,
        );
        revision = saved?.revision ?? 0;
        progress = session.getProgress();
        chosenSerial = progress.selectedSerial ?? "";
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
    This is a guarded handoff guide, not completed automatic factory
    provisioning. The app does not modify Linux, Wi-Fi, SSH keys, the
    bootloader, or the OS here. Existing unrelated configuration is preserved.
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
      <p>Checkpoint {progress.stageIndex + 1} of {setupSteps.length}</p>
      <h4>{step.title}</h4>
      <p>{step.instruction}</p>
      {#if step.kind === "unsupported"}
        <p class="error">
          Pending implementation — this checkpoint cannot be marked complete.
          Full Pairing, atomic deployment/rollback, and their setup acceptance
          remain blocked.
        </p>
        <button onclick={onUseProvisioned}
          >Return to already-provisioned Board connections</button
        >
      {:else}
        <fieldset disabled={!native || busy || !progress.selectedSerial}>
          {#if step.kind !== "usb"}
            <label class="check"
              ><input type="checkbox" bind:checked={confirmed} />I completed
              this step for the selected UNO Q in person / in official Arduino
              tooling (or explicitly chose the described USB-only skip)</label
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
              : item.kind === "unsupported"
                ? "pending implementation"
                : "not yet recorded"}
          </li>
        {/each}
      </ol>
    </details>
    <button disabled={!native || busy} onclick={() => act(() => {})}
      >Retry saving checkpoints</button
    >
  {/if}
</section>

<style>
  .setup {
    border: 1px solid #b4bdab;
    border-radius: 12px;
    padding: 20px;
    margin: 16px 0;
    background: #fffcf5;
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
    border: 1px solid #ccc;
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
    color: white;
    background: #173b35;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .error {
    padding: 12px;
    color: #84221b;
    background: #ffe8df;
    border-radius: 8px;
  }
  summary {
    cursor: pointer;
  }
  li {
    margin: 8px 0;
  }
</style>
