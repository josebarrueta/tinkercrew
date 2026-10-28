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
    parentTask,
    continueLabel = "Continue to Mission",
    onBusyChange = () => {},
  }: {
    visible: boolean;
    onConnected: (client: BoardClient) => void;
    onSimulator: () => void;
    onState: (state: ConnectionState) => void;
    onMission?: () => void;
    advanced?: boolean;
    defer?: boolean;
    onParent?: () => void;
    parentTask?:
      "overview" | "setup" | "network" | "updates" | "troubleshooting";
    continueLabel?: string;
    onBusyChange?: (busy: boolean) => void;
  } = $props();
  let usbBoards = $state<{ serial: string }[]>([]);
  let selectedUsb = $state("");
  let usbHandle: number | undefined;
  let boards = $state<{ boardId: string; name: string }[]>([]);
  let selected = $state("");
  let record = $state("");
  let usbConfirmed = $state(false);
  let setupOpen = $state(false);
  let maintenanceOpen = $state(false);
  let networkOpen = $state(false);
  $effect(() => {
    if (parentTask) {
      maintenanceOpen = [
        "setup",
        "updates",
        "troubleshooting",
        "network",
      ].includes(parentTask);
      networkOpen = parentTask === "network";
    }
  });
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
        void connectSaved(true);
      }, 5000);
  }

  async function connectSaved(retrying = false) {
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

  async function closeUsb(handle = usbHandle) {
    if (handle === usbHandle) usbHandle = undefined;
    if (native && handle !== undefined)
      await invoke("disconnect_play_handle", { handle }).catch(() => {});
  }

  async function connectUsb() {
    if (!native || !selectedUsb || busy || managementBusy || disposed) return;
    const epoch = ++intent;
    const current = () => !disposed && epoch === intent;
    busy = true;
    reconnect = false;
    clearTimeout(retry);
    message = "";
    client?.close();
    report({ phase: "searching", stage: "usb" });
    try {
      await invoke("disconnect_board");
      if (!current()) return;
      const result = await invoke<{ handle: number; boardId: string }>(
        "connect_usb_play",
        { serial: selectedUsb },
      );
      if (!current()) {
        await closeUsb(result.handle);
        return;
      }
      usbHandle = result.handle;
      client = await ConnectedBoardClient.open(
        result.handle,
        result.boardId,
        (handle, operation) => invoke("board_request", { handle, operation }),
        (error) => {
          if (!current()) return;
          report({ phase: "failed", error });
          void closeUsb();
        },
      );
      if (!current()) {
        client.close();
        await closeUsb(result.handle);
        return;
      }
      connectedBoardId = result.boardId;
      onConnected(client);
      report({ phase: "connected", transport: "usb" });
    } catch (cause) {
      await closeUsb();
      if (current()) report({ phase: "failed", error: connectionError(cause) });
    } finally {
      busy = false;
    }
  }

  export async function cancelPending() {
    if (
      !busy &&
      !loadingBoards &&
      connection.phase !== "searching" &&
      !(reconnect && connection.phase === "failed")
    )
      return;
    const preserveConnected = connection.phase === "connected" && !busy;
    ++intent;
    reconnect = false;
    clearTimeout(retry);
    if (preserveConnected) return;
    client?.close();
    onSimulator();
    report({ phase: "simulator" });
    await closeUsb();
    // In-flight discovery/connect retains its busy gate until it settles.
    // Its epoch checks dispose only its own late USB handle.
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
    if (
      !native ||
      disposed ||
      busy ||
      loadingBoards ||
      managementBusy ||
      connection.phase === "connected"
    )
      return;
    const epoch = ++intent;
    loadingBoards = true;
    message = "";
    try {
      const detected = await invoke<typeof usbBoards>("usb_play_boards");
      if (disposed || epoch !== intent) return;
      usbBoards = detected;
      if (detected.length === 1) {
        selectedUsb = detected[0].serial;
        loadingBoards = false;
        await connectUsb();
      } else if (!detected.length) {
        message =
          "No ready USB Board detected. Check the data cable and allow the Board to finish booting, then detect again.";
      }
    } catch (cause) {
      if (!disposed && epoch === intent)
        report({ phase: "failed", error: connectionError(cause) });
    } finally {
      loadingBoards = false;
    }
  }

  async function openSavedBoards() {
    if (
      !native ||
      disposed ||
      busy ||
      loadingBoards ||
      connection.phase === "connected"
    )
      return;
    const epoch = ++intent;
    loadingBoards = true;
    try {
      const saved = await invoke<typeof boards>("list_boards");
      if (disposed || epoch !== intent) return;
      boards = saved;
      if (boards.length === 1) selected = boards[0].boardId;
    } catch (cause) {
      if (!disposed && epoch === intent)
        message = connectionError(cause).message;
    } finally {
      loadingBoards = false;
    }
  }
  onMount(() => {
    // Even the legacy screen must not silently look up credentials or connect.
    return () => {
      disposed = true;
      ++intent;
      reconnect = false;
      clearTimeout(retry);
      client?.close();
      void closeUsb();
    };
  });
</script>

<section hidden={!visible} class="connections">
  <h2 tabindex="-1">
    {parentTask && parentTask !== "overview"
      ? {
          setup: "Board setup",
          network: "Network & saved permission",
          updates: "Software updates",
          troubleshooting: "Troubleshooting",
        }[parentTask]
      : advanced
        ? "Connect & play"
        : "Connect your Board"}
  </h2>
  <p>
    Plug in a prepared UNO Q with a USB data cable. USB play uses temporary
    permission in this app's memory — no Mac or Linux password, Keychain access,
    or saved Pairing. Connect never resets, restarts or uploads firmware. A new
    Board needs parent setup once.
  </p>
  <p>
    Practice never drives the connected Board. Choose a module test or Run
    explicitly when you are ready to use real hardware.
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
    {:else}Practice mode is active.{/if}
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
  <fieldset
    hidden={!!parentTask}
    disabled={!native ||
      busy ||
      loadingBoards ||
      managementBusy ||
      connection.phase === "connected"}
  >
    <legend>USB play</legend>
    <label
      >USB Board
      <select bind:value={selectedUsb}>
        <option value="">Select the connected USB Board</option>
        {#each usbBoards as board}<option value={board.serial}
            >{board.serial}</option
          >{/each}
      </select>
    </label>
    <button onclick={openConnection}>Detect USB Board</button>
    <button disabled={!selectedUsb} onclick={connectUsb}
      >Connect / retry USB Board</button
    >
  </fieldset>
  {#if !parentTask || connection.phase === "connected"}<button
      onclick={() => useSimulator()}>Use Practice</button
    >{/if}
  {#if connection.phase === "connected"}<p>
      Connection details are read-only. Use Practice to disconnect before
      choosing another Board or network.
    </p>{/if}
  {#if advanced}<details
      bind:open={networkOpen}
      hidden={!!parentTask && parentTask !== "network"}
    >
      <summary>Parent network connection — saved Pairing (optional)</summary>
      <p>
        Wi-Fi play uses saved Mac credentials and can require Keychain approval.
        USB play above does not read those credentials or fall back to the
        network.
      </p>
      <button
        disabled={!native ||
          busy ||
          loadingBoards ||
          managementBusy ||
          connection.phase === "connected"}
        onclick={openSavedBoards}>Load saved Boards for network play</button
      >
      <fieldset
        disabled={!native ||
          busy ||
          loadingBoards ||
          managementBusy ||
          connection.phase === "connected"}
      >
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
            ><input type="checkbox" bind:checked={localConsent} />Allow checking
            up to eight cached local neighbors after Bonjour fails (no subnet
            scan)</label
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
        <button disabled={!selected} onclick={() => connectSaved()}
          >Connect / retry selected Board</button
        >
        <button hidden={!!parentTask} onclick={() => useSimulator()}
          >Use Practice</button
        >
      </fieldset>
    </details>{/if}
  {#if advanced && !parentTask}<button
      class="primary"
      disabled={busy ||
        managementBusy ||
        connection.phase === "searching" ||
        connection.phase === "failed"}
      onclick={onMission}>{continueLabel}</button
    >{/if}
  {#if !parentTask}<button onclick={openParentSetup}
      >Parent setup (one time)</button
    >{/if}
  <p>
    Opening a Mission does not enable physical lights or sound. Mission controls
    need a separate explicit action.
  </p>
  {#if advanced}<details
      bind:open={setupOpen}
      hidden={!!parentTask && parentTask !== "setup"}
    >
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
        area={parentTask === "overview" ? undefined : parentTask}
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
          onBusyChange(value);
        }}
      />
    </details>
    <HardwareSafety />
    <details hidden={!!parentTask && parentTask !== "setup"}>
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
