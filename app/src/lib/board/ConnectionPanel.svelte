<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
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
    onMission = () => {},
    advanced = true,
    defer = false,
    onParent = () => {},
  }: {
    visible: boolean;
    onConnected: (client: BoardClient) => void;
    onSimulator: () => void;
    onState: (state: ConnectionState) => void;
    onMission?: () => void;
    advanced?: boolean;
    defer?: boolean;
    onParent?: () => void;
  } = $props();
  let boards = $state<{ boardId: string; name: string }[]>([]);
  let selected = $state("");
  let record = $state("");
  let usbConfirmed = $state(false);
  let setupOpen = $state(false);
  let maintenanceOpen = $state(false);
  let maintenanceSection = $state<HTMLDetailsElement>();
  async function openParentSetup() {
    if (!advanced) {
      onParent();
      return;
    }
    maintenanceOpen = true;
    await tick();
    maintenanceSection?.scrollIntoView({ block: "start" });
  }
  let localConsent = $state(false);
  let manualHost = $state("");
  let manualPort = $state(7443);
  let busy = $state(false);
  let loadingBoards = $state(false);
  let intent = 0;
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
    const epoch = ++intent;
    const current = () => !disposed && epoch === intent;
    const requireCurrent = () => {
      if (!current())
        throw { code: "operation_cancelled", message: "Connection cancelled." };
    };
    const scopedBackend: ConnectionBackend = {
      candidates: async (...args) => {
        requireCurrent();
        const value = await backend.candidates(...args);
        requireCurrent();
        return value;
      },
      probe: async (...args) => {
        requireCurrent();
        const value = await backend.probe(...args);
        requireCurrent();
        return value;
      },
    };
    reconnectBoardId = boardId;
    busy = true;
    reconnect = true;
    message = "";
    clearTimeout(retry);
    client?.close();
    try {
      await invoke("disconnect_board");
      requireCurrent();
      const result = await connectBoard(
        scopedBackend,
        {
          boardId,
          localConsent,
          ...(manualHost.trim()
            ? { manual: { host: manualHost.trim(), port: manualPort } }
            : {}),
        },
        (next) => {
          if (current()) report(next);
        },
      );
      if (!current()) {
        await invoke("disconnect_board");
        return;
      }
      client = await ConnectedBoardClient.open(
        result.handle,
        boardId,
        (handle, operation) => invoke("board_request", { handle, operation }),
        (error) => {
          if (!current()) return;
          report({ phase: "failed", error });
          if (
            [
              "authentication_failed",
              "authentication_busy",
              "keychain_unavailable",
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
      if (!current()) {
        client.close();
        await invoke("disconnect_board");
        return;
      }
      connectedBoardId = boardId;
      onConnected(client);
      report({ phase: "connected", transport: result.transport });
    } catch (cause) {
      if (!current()) {
        await invoke("disconnect_board").catch(() => {});
        return;
      }
      const error = connectionError(cause);
      report({ phase: "failed", error });
      if (
        [
          "authentication_failed",
          "authentication_busy",
          "keychain_unavailable",
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

  export async function useSimulator() {
    ++intent;
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

  export async function openConnection() {
    if (!native || disposed || busy || loadingBoards) return;
    const epoch = ++intent;
    loadingBoards = true;
    try {
      const saved = await invoke<typeof boards>("list_boards");
      if (disposed || epoch !== intent) return;
      boards = saved;
      if (boards.length === 1) {
        selected = boards[0].boardId;
        await connect();
      }
    } catch (cause) {
      if (!disposed && epoch === intent)
        message = connectionError(cause).message;
    } finally {
      loadingBoards = false;
    }
  }
  onMount(() => {
    if (!defer) void openConnection();
    return () => {
      disposed = true;
      ++intent;
      reconnect = false;
      clearTimeout(retry);
      client?.close();
    };
  });
</script>

<section hidden={!visible} class="connections">
  <h2>{advanced ? "Connect & play" : "Connect your Board"}</h2>
  {#if !advanced}<p>
      This Mac stores only its own Board permission in macOS secure storage, not
      your other passwords. Approve Tinkeroo's expected OS prompt if shown. No
      firmware, restart or reset runs from Connect.
    </p>{/if}
  <p>
    {advanced
      ? "Already paired? Connect your Board below, then continue to your Mission."
      : "Choose your saved Board and connect. New Board? Open parent setup once."}
    No Linux password or repeated setup is needed.
  </p>
  <p>
    Using the simulator instead? It is practice only and never drives the
    connected Board.
  </p>
  {#if !native}<p>
      Board connections are available in the native macOS app. This browser
      preview stays in simulator mode.
    </p>{/if}
  <p role="status">
    {#if connection.phase === "searching"}Looking for the selected Board: {connection.stage}…
    {:else if connection.phase === "connected"}Connected to {boards.find(
        (board) => board.boardId === connectedBoardId,
      )?.name ?? "your Board"} via
      {connection.transport === "usb" ? "USB" : "a verified local connection"}.
    {:else if connection.phase === "failed"}{#if advanced}{connection.error
          .code}:
      {/if}{connection.error.message}
    {:else}Simulator mode is active.{/if}
  </p>
  {#if message}<p role="status">{message}</p>{/if}
  {#if !advanced && connection.phase === "failed"}<p>
      If the Board is visible over USB but its Companion is stopped, use
      official Arduino App Lab to start the prepared Tinkeroo Companion.
      Official startup may compile/upload firmware and stop another Board app:
      review that action first. Connect never does it silently. Do not reset or
      re-pair merely because a connection failed.
    </p>{/if}
  {#if managementBusy}<p role="status">
      Parent maintenance is active or needs recovery. Gameplay stays paused;
      open Parent setup &amp; advanced maintenance below to inspect it.
    </p>{/if}
  <fieldset disabled={!native || busy || loadingBoards || managementBusy}>
    <legend>Your saved Board</legend>
    <label
      >Paired Board
      <select bind:value={selected}>
        <option value="">Select a Board identity</option>
        {#each boards as board}<option value={board.boardId}
            >{board.name} · {board.boardId.slice(-6)}</option
          >{/each}
      </select>
    </label>
    <details>
      <summary>Connection options (optional)</summary>
      <label class="check"
        ><input type="checkbox" bind:checked={localConsent} />Allow checking up
        to eight cached local neighbors after Bonjour fails (no subnet scan)</label
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
      <p>
        Try USB if local-network or VPN settings block the Board. The same
        pinned identity checks apply to every route.
      </p>
    </details>
    <button disabled={!selected} onclick={() => connect()}
      >Connect / retry selected Board</button
    >
    <button onclick={() => useSimulator()}
      >Use simulator / stop reconnecting</button
    >
  </fieldset>
  {#if advanced}<button
      class="primary"
      disabled={busy ||
        managementBusy ||
        connection.phase === "searching" ||
        connection.phase === "failed"}
      onclick={onMission}>Continue to Mission</button
    >{/if}
  <button onclick={openParentSetup}>Add / pair a Board</button>
  {#if native && boards.length === 0}<p>
      No saved Board yet. Pair this Mac once, then use Connect &amp; play for
      later sessions.
    </p>{/if}
  <p>
    Opening a Mission does not enable physical lights or sound. Mission controls
    need a separate explicit action.
  </p>
  {#if advanced}<button
      disabled={!native || busy || loadingBoards}
      onclick={openConnection}>Load saved Boards to connect</button
    >
    <details bind:open={setupOpen}>
      <summary>Set up a UNO Q — resumable official-tool handoff guide</summary>
      <p>
        Use this for a new Board, not every play session. Already paired Boards
        can use Connect &amp; play above.
      </p>
      <button onclick={openParentSetup}
        >Open parent setup and Pairing controls</button
      >
      {#if setupOpen && visible}<SetupWizard
          beforeChange={useSimulator}
          onProofRefresh={async (boardId) => {
            boards = await invoke("list_boards");
            if (boardId && boards.some((board) => board.boardId === boardId))
              selected = boardId;
          }}
          onUseProvisioned={async () => {
            setupOpen = false;
            if (native) boards = await invoke("list_boards");
          }}
        />{/if}
    </details>
    <details bind:open={maintenanceOpen} bind:this={maintenanceSection}>
      <summary>Parent setup &amp; advanced maintenance</summary>
      <p>
        Pair a new Board here. Linux passwords, Wi-Fi changes, updates and
        recovery are separate adult-only tasks; they are not prerequisites for
        opening a Mission.
      </p>
      <BoardManagement
        visible={visible && maintenanceOpen}
        pairedBoardId={selected}
        beforeChange={useSimulator}
        onForgot={async () => {
          boards = await invoke("list_boards");
          selected = boards.length === 1 ? boards[0].boardId : "";
          message =
            "Your saved Board list is up to date. Select a Board and Connect; Pairing alone does not connect it.";
        }}
        onBusyChange={(value) => {
          managementBusy = value;
        }}
      />
    </details>
    <HardwareSafety />
    <details>
      <summary>Import an already provisioned Board (development bridge)</summary
      >
      <p>
        Legacy development import only. Most families should use Add / pair a
        Board instead. Copy a legacy record only from the selected Board over
        physical USB; never accept credentials or certificates learned only
        through Wi-Fi discovery.
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
        <button
          disabled={!usbConfirmed || !record}
          onclick={() => importRecord()}>Save record to Keychain</button
        >
      </fieldset>
    </details>
    <p>
      No Linux password, SSH key, or token is stored in Mission progress or
      browser storage. UNO Q system updates are never automatic.
    </p>{/if}
</section>

<style>
  .primary {
    background: var(--primary);
    color: white;
    font-weight: 800;
  }
  .connections {
    margin-top: 24px;
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
