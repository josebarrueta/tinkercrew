<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import {
    connectBoard,
    connectionError,
    type ConnectionBackend,
    type ConnectionState,
  } from "./connection";
  import { ConnectedBoardClient } from "./connected";
  import type { BoardClient } from "./types";
  import HardwareSafety from "./HardwareSafety.svelte";
  import SetupWizard from "../setup/SetupWizard.svelte";
  import BoardManagement from "./BoardManagement.svelte";

  let {
    visible,
    onConnected,
    onSimulator,
    onState,
  }: {
    visible: boolean;
    onConnected: (client: BoardClient) => void;
    onSimulator: () => void;
    onState: (state: ConnectionState) => void;
  } = $props();
  let boards = $state<{ boardId: string; name: string }[]>([]);
  let selected = $state("");
  let record = $state("");
  let usbConfirmed = $state(false);
  let setupOpen = $state(false);
  let localConsent = $state(false);
  let manualHost = $state("");
  let manualPort = $state(7443);
  let busy = $state(false);
  let managementBusy = $state(false);
  let connection = $state<ConnectionState>({ phase: "simulator" });
  let message = $state("");
  let client: ConnectedBoardClient | undefined;
  let retry: ReturnType<typeof setTimeout> | undefined;
  let reconnect = false;
  let reconnectBoardId = "";
  let connectedBoardId = $state("");
  let disposed = false;
  const native = isTauri();
  const backend: ConnectionBackend = {
    candidates: (boardId, stage, localConsent) =>
      invoke("board_candidates", { boardId, stage, localConsent }),
    probe: (boardId, candidate) =>
      invoke("probe_board", { boardId, candidate }),
  };

  function report(next: ConnectionState) {
    connection = next;
    onState(next);
  }
  function scheduleRetry() {
    clearTimeout(retry);
    if (reconnect && !disposed)
      retry = setTimeout(() => {
        void connect(true);
      }, 5000);
  }

  async function connect(retrying = false) {
    const boardId = retrying ? reconnectBoardId : selected;
    if (!native || !boardId || busy || managementBusy || disposed) return;
    reconnectBoardId = boardId;
    busy = true;
    reconnect = true;
    message = "";
    clearTimeout(retry);
    client?.close();
    try {
      await invoke("disconnect_board");
      const result = await connectBoard(
        backend,
        {
          boardId,
          localConsent,
          ...(manualHost.trim()
            ? { manual: { host: manualHost.trim(), port: manualPort } }
            : {}),
        },
        report,
      );
      if (disposed) {
        await invoke("disconnect_board");
        return;
      }
      client = await ConnectedBoardClient.open(
        result.handle,
        boardId,
        (handle, operation) => invoke("board_request", { handle, operation }),
        (error) => {
          report({ phase: "failed", error });
          if (
            [
              "authentication_failed",
              "pairing_required",
              "identity_mismatch",
              "protocol_incompatible",
              "maintenance_active",
            ].includes(error.code)
          )
            reconnect = false;
          scheduleRetry();
        },
      );
      connectedBoardId = boardId;
      onConnected(client);
      report({ phase: "connected", transport: result.transport });
    } catch (cause) {
      const error = connectionError(cause);
      report({ phase: "failed", error });
      if (
        [
          "authentication_failed",
          "pairing_required",
          "protocol_incompatible",
          "maintenance_active",
        ].includes(error.code)
      )
        reconnect = false;
      scheduleRetry();
    } finally {
      busy = false;
    }
  }

  async function useSimulator() {
    reconnect = false;
    clearTimeout(retry);
    client?.close();
    onSimulator();
    report({ phase: "simulator" });
    if (native) await invoke("disconnect_board").catch(() => {});
  }

  async function importRecord() {
    if (!native || busy || managementBusy) return;
    busy = true;
    try {
      await invoke("register_board", { record, usbConfirmed });
      record = "";
      usbConfirmed = false;
      boards = await invoke("list_boards");
      if (boards.length === 1) selected = boards[0].boardId;
      message =
        "USB-verified Board record saved in macOS Keychain. Select its identity and connect.";
    } catch (cause) {
      message = connectionError(cause).message;
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    if (native)
      void (async () => {
        try {
          boards = await invoke("list_boards");
          if (!disposed && boards.length === 1) {
            selected = boards[0].boardId;
            await connect();
          }
        } catch (cause) {
          message = connectionError(cause).message;
        }
      })();
    return () => {
      disposed = true;
      reconnect = false;
      clearTimeout(retry);
      client?.close();
    };
  });
</script>

<section hidden={!visible} class="connections">
  <h2>Board Setup</h2>
  <p>
    One place for Board connections, Linux configuration, restart, and explicit
    recovery. Destructive actions are for an adult and are separate from Mission
    play.
  </p>
  <HardwareSafety />
  <BoardManagement
    {visible}
    pairedBoardId={selected}
    beforeChange={useSimulator}
    onForgot={async () => {
      boards = await invoke("list_boards");
      selected = "";
    }}
    onBusyChange={(value) => {
      managementBusy = value;
    }}
  />
  <details bind:open={setupOpen}>
    <summary>Set up a UNO Q — resumable official-tool handoff guide</summary>
    {#if setupOpen}<SetupWizard
        onUseProvisioned={() => {
          setupOpen = false;
        }}
      />{/if}
  </details>
  <p>
    Simulator mode needs no account or internet. Connected play uses a
    provisioned Companion, TLS verification, and a Board-specific credential
    held in macOS Keychain.
  </p>
  {#if !native}<p>
      Board connections are available in the native macOS app. This browser
      preview stays in simulator mode.
    </p>{/if}
  <p role="status">
    {#if connection.phase === "searching"}Looking for the selected Board: {connection.stage}…
    {:else if connection.phase === "connected"}Connected to {connectedBoardId} through
      {connection.transport}.
    {:else if connection.phase === "failed"}{connection.error.code}: {connection
        .error.message}
    {:else}Simulator mode is active.{/if}
  </p>
  {#if message}<p role="status">{message}</p>{/if}
  <fieldset disabled={!native || busy || managementBusy}>
    <legend>Companion gameplay connection — choose the exact Board</legend>
    <label
      >Paired Board
      <select bind:value={selected}>
        <option value="">Select a Board identity</option>
        {#each boards as board}<option value={board.boardId}
            >{board.name} — {board.boardId}</option
          >{/each}
      </select>
    </label>
    <label class="check"
      ><input type="checkbox" bind:checked={localConsent} />Allow checking up to
      eight cached local neighbors after Bonjour fails (no subnet scan)</label
    >
    <label
      >Manual local IP or hostname (optional)<input
        bind:value={manualHost}
        placeholder="192.168.1.42 or dragon.local"
      /></label
    >
    <label
      >Companion port<input
        type="number"
        min="1"
        max="65535"
        bind:value={manualPort}
      /></label
    >
    <button disabled={!selected} onclick={() => connect()}
      >Connect / retry selected Board</button
    >
    <button onclick={() => useSimulator()}
      >Use simulator / stop reconnecting</button
    >
  </fieldset>
  <p>
    The connection order is last validated address → Bonjour → parent-approved
    local neighbors → serial-targeted USB/ADB → manual entry. TLS and Board
    identity checks apply to every path, including USB. If a VPN blocks local
    routing, allow local-network access in its settings or use USB.
  </p>
  <details>
    <summary>Import an already provisioned Board (development bridge)</summary>
    <p>
      This is not automatic first-time Pairing. Copy the record directly from
      the selected Board over physical USB; never accept a certificate or
      credential learned only from Wi-Fi discovery. Official Arduino setup
      remains the fallback. Full TBP USB key registration and short-lived
      sessions remain integration work.
    </p>
    <fieldset disabled={!native || busy || managementBusy}>
      <label
        >USB-verified Board record (contains a secret)<textarea
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          rows="6"
          bind:value={record}></textarea></label
      >
      <label class="check"
        ><input type="checkbox" bind:checked={usbConfirmed} />I verified the
        serial, Board ID, and certificate directly over physical USB</label
      >
      <button disabled={!usbConfirmed || !record} onclick={() => importRecord()}
        >Save record to Keychain</button
      >
    </fieldset>
  </details>
  <p>
    No Linux password, SSH key, or token is stored in Mission progress or
    browser storage. UNO Q system updates are never automatic.
  </p>
</section>

<style>
  .connections {
    margin-top: 30px;
    max-width: 1000px;
  }
  h2 {
    letter-spacing: -1px;
    font-size: 42px;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: 12px;
    margin: 20px 0;
    padding: 20px;
  }
  label {
    display: block;
    margin: 12px 0;
  }
  input,
  select,
  textarea {
    display: block;
    width: 100%;
    font: inherit;
    margin-top: 7px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  textarea {
    -webkit-text-security: disc;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .check input {
    width: auto;
  }
  button {
    font: inherit;
    padding: 12px;
    margin: 6px 10px 6px 0;
    background: var(--surface);
    color: var(--primary);
    border: 1px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  summary {
    cursor: pointer;
    font-weight: bold;
  }
</style>
