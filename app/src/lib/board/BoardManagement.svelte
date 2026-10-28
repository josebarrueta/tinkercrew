<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { errorMessage } from "../errors";
  import type { UsbDevice } from "../setup/session";

  type Identity = {
    adbSerial: string;
    serial: string;
    username: string;
    hostname: string;
    os: string;
    osVersion: string;
    imageVersion: string;
  };
  type Image = { version: string; os: string; url: string; sha256: string };
  type Plan = { planId: number; board: Identity; image: Image };
  type RestoreStatus = { phase: string; bytes: number; message: string };
  let {
    visible,
    beforeChange,
    pairedBoardId,
    onForgot,
    onBusyChange,
    area,
  }: {
    visible: boolean;
    area?: "setup" | "network" | "updates" | "troubleshooting";
    beforeChange: () => Promise<void>;
    pairedBoardId: string;
    onForgot: () => Promise<void>;
    onBusyChange: (busy: boolean) => void;
  } = $props();
  const native = isTauri();
  let devices = $state<UsbDevice[]>([]);
  let serial = $state("");
  let board = $state<Identity | null>(null);
  let authenticated = $state(false);
  let username = $state("arduino");
  let password = $state("");
  let usbConfirmed = $state(false);
  let enableConfirmation = $state("");
  let hostname = $state("");
  let ssids = $state<string[]>([]);
  let ssid = $state("");
  let wifiPassword = $state("");
  let restartConfirmation = $state("");
  let restartLanHost = $state("");
  let restartLocalConsent = $state(false);
  let forgetConfirmation = $state("");
  let trustConfirmation = $state("");
  let initialRelease = $state("");
  let initialStatus = $state("");
  let prepareConfirmation = $state("");
  let cancelPrepareConfirmation = $state("");
  let correctPreparedConfirmation = $state("");
  let restorePreparedConfirmation = $state("");
  let renewIdentityConfirmation = $state("");
  let appVersion = $state("native app required");
  let runtimeConfirmation = $state("");
  let runtimeAbortConfirmation = $state("");
  let runtimeStatus = $state("");
  let runtimeRelease = $state("");
  let pairConfirmation = $state("");
  let installationLabel = $state("Family Mac");
  let pairingSerial = $state("");
  let pairings = $state<
    { installation_id: string; label: string; state: string }[]
  >([]);
  let revokeInstallation = $state("");
  let revokeConfirmation = $state("");
  let tool = $state("");
  let images = $state<Image[]>([]);
  let imageIndex = $state("");
  let plan = $state<Plan | null>(null);
  let eraseConfirmation = $state("");
  let backup = $state(false);
  let edl = $state(false);
  let onlyTarget = $state(false);
  let power = $state(false);
  let verifyConfirmation = $state("");
  let officialRecoveryConfirmed = $state(false);
  let restore = $state<RestoreStatus>({
    phase: "idle",
    bytes: 0,
    message: "No factory restore is running.",
  });
  let busy = $state(false);
  let error = $state("");
  let recoveryOpen = $state(false);
  let message = $state("");
  const restorePending = $derived(!["idle", "failed"].includes(restore.phase));
  const canErase = $derived(
    native &&
      !busy &&
      !restorePending &&
      plan !== null &&
      eraseConfirmation === `ERASE ${plan.board.serial}` &&
      backup &&
      edl &&
      onlyTarget &&
      power,
  );
  const chosenImage = $derived(images[Number(imageIndex)]);
  $effect(() => {
    if (restorePending && area) recoveryOpen = true;
  });

  $effect(() => {
    onBusyChange(busy || restorePending);
  });
  $effect(() => {
    if (!visible) {
      password = "";
      wifiPassword = "";
      authenticated = false;
      if (native) void invoke("admin_logout").catch(() => {});
    }
  });
  async function task(action: () => Promise<void>, changing = false) {
    if (!native || busy) return;
    busy = true;
    error = "";
    message = "";
    try {
      if (changing) await beforeChange();
      await action();
    } catch (cause) {
      error = errorMessage(
        cause,
        "Board operation failed. Check identity, credentials, and supported tooling; success has not been assumed.",
      );
    } finally {
      password = "";
      wifiPassword = "";
      busy = false;
    }
  }
  async function inspect() {
    board = await invoke("admin_inspect_usb", { serial });
    hostname = board!.hostname;
    message = `Physically inspected UNO Q: hardware serial ${board!.serial}. This read-only check does not authenticate your Linux password.`;
  }
  async function action(operation: Record<string, unknown>) {
    const result = await invoke<Record<string, unknown>>("admin_action", {
      serial,
      action: operation,
    });
    message =
      operation.operation === "reboot"
        ? "Restart scheduled for about one minute from now. Wait for boot and reconnect; scheduling is not verified restart completion."
        : operation.operation === "wifi_join"
          ? "NetworkManager reported a successful Wi-Fi connection. Recheck LAN connectivity before depending on it."
          : operation.operation === "hostname"
            ? "Hostname updated. The LAN address/discovery name may change; USB remains available."
            : "Board operation completed.";
    if (operation.operation === "wifi_scan") ssids = result.ssids as string[];
    if (operation.operation === "status") board = result as unknown as Identity;
    if (operation.operation === "reboot") {
      authenticated = false;
      restartConfirmation = "";
    }
  }
  async function openResource(resource: "recovery" | "flasher_download") {
    const urls = {
      recovery: "https://docs.arduino.cc/software/app-lab/configure/flash/",
      flasher_download: "https://www.arduino.cc/en/software/#flasher-tool",
    };
    try {
      if (native) await invoke("open_board_resource", { resource });
      else window.open(urls[resource], "_blank", "noopener,noreferrer");
    } catch {
      error = `Open the official resource in your browser: ${urls[resource]}`;
    }
  }
  async function pollRestore() {
    if (!native || busy) return;
    try {
      restore = await invoke("factory_restore_status");
    } catch {
      error =
        "Recovery status could not be read. Do not unplug or start another flash while the outcome is unknown.";
    }
  }
  onMount(() => {
    void pollRestore();
    if (native)
      void invoke<{ release: string }>("initial_companion_package")
        .then((value) => {
          initialRelease = value.release;
        })
        .catch(() => {
          initialRelease = "unavailable";
        });
    if (native)
      void invoke<{ version: string }>("app_update_info")
        .then((value) => {
          appVersion = value.version;
        })
        .catch(() => {
          appVersion = "unavailable";
        });
    if (native)
      void invoke<{ release: string }>("runtime_bundle_info")
        .then((value) => {
          runtimeRelease = value.release;
        })
        .catch(() => {
          error =
            "The fixed runtime bundle is unavailable; no update can be started.";
        });
    const timer = setInterval(() => {
      if (restore.phase !== "idle") void pollRestore();
    }, 1000);
    return () => clearInterval(timer);
  });
  onDestroy(() => {
    password = "";
    wifiPassword = "";
  });
</script>

<section class="management" aria-label="Board configuration and recovery">
  <details hidden={!!area && area !== "updates"}>
    <summary>Upgrade the Mac app</summary>
    <p>
      Installed Mac app: {appVersion}. App upgrades are manual and
      parent-approved, separate from the UNO Q runtime updater below. There is
      no background updater or automatic download.
    </p>
    <ol>
      <li>
        Finish any Board maintenance, stop the Mission, and disconnect gameplay.
        The button below returns this Mac to Simulator mode.
      </li>
      <li>
        Choose a newer supported release from the official GitHub Releases page.
        Public signed/notarized installers are not available/validated yet;
        development builds may share a version number, so check their commit and
        architecture.
      </li>
      <li>
        Quit Tinkeroo before replacing Tinkeroo.app in Applications from the new
        DMG. Keep this Mac's app data and Keychain entries; do not use Forget,
        delete progress, or factory-reset the Board to upgrade.
      </li>
      <li>
        Reopen Tinkeroo, check your Crew/progress, and explicitly reconnect and
        rearm only when ready. Never bypass Gatekeeper. If blocked, stop and use
        a trusted local build or wait for an approved release.
      </li>
    </ol>
    <button
      disabled={!native || busy || restorePending}
      onclick={() =>
        task(async () => {
          await invoke("open_app_releases");
          message =
            "Requested the official Mac app releases page. No app was downloaded or installed, and the Board was not updated. Quit Tinkeroo before replacing the app.";
        }, true)}>Open official Mac app releases</button
    >
    <p>
      The README includes source-build upgrade commands. Replacing the Mac app
      never automatically upgrades Board firmware or the Companion.
    </p>
  </details>
  <h3>Parent setup and maintenance</h3>
  <p>
    Start by selecting and inspecting the physical USB Board. Installation
    Pairing does not need a Linux password. The separate Linux sign-in is only
    for tasks such as changing Wi-Fi or hostname; it is not part of normal
    Mission play. Passwords are used for one request and then cleared.
    Management pauses gameplay.
  </p>
  {#if !native}<p class="notice">
      Browser preview only. Configuration, restart, and factory restore are
      disabled; use the native macOS app.
    </p>{/if}
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <fieldset disabled={!native || busy || restorePending}>
    <legend>1. Select and inspect the exact USB Board</legend>
    <button
      onclick={() =>
        task(async () => {
          devices = await invoke("inspect_setup_usb");
        })}>Refresh USB Boards</button
    >
    <label
      >USB Board serial
      <select
        value={serial}
        onchange={(event) => {
          serial = event.currentTarget.value;
          board = null;
          authenticated = false;
          usbConfirmed = false;
          plan = null;
          password = "";
          wifiPassword = "";
          enableConfirmation = "";
          restartConfirmation = "";
        }}
      >
        <option value=""
          >Choose explicitly — never the first device automatically</option
        >
        {#each devices as device}<option value={device.serial}
            >{device.serial} — {device.state}</option
          >{/each}
      </select>
    </label>
    <button disabled={!serial} onclick={() => task(inspect)}
      >Inspect selected UNO Q</button
    >
    {#if board}<p>
        UNO Q {board.serial} · {board.hostname} · {board.os}
        {board.osVersion} · image {board.imageVersion}
      </p>{/if}
    <label class="check"
      ><input type="checkbox" bind:checked={usbConfirmed} />I physically
      verified this USB Board and its displayed serial</label
    >
  </fieldset>
  <fieldset
    hidden={area === "setup" || area === "updates"}
    disabled={!native || busy || restorePending || !board || !usbConfirmed}
  >
    <legend>2. Sign in with existing Linux credentials</legend>
    <label
      >Linux username<input
        autocomplete="off"
        maxlength="32"
        bind:value={username}
      /></label
    >
    <label
      >Linux password (this request only)<input
        type="password"
        autocomplete="off"
        maxlength="512"
        bind:value={password}
      /></label
    >
    <button
      class="primary"
      disabled={!password || !username}
      onclick={() =>
        task(async () => {
          board = await invoke("admin_login", {
            serial,
            username,
            password,
            usbConfirmed,
          });
          authenticated = true;
          message =
            "Linux authentication and physical Board identity verified. Actions are authorized for five minutes; the password was not stored. Re-enter it for privileged changes.";
        }, true)}>Authenticate selected Board</button
    >
    <button
      disabled={!authenticated}
      onclick={() =>
        task(async () => {
          await invoke("admin_logout");
          authenticated = false;
          message = "Management session closed.";
        })}>Sign out of management</button
    >
    <details>
      <summary>SSH is not enabled yet</summary>
      <p>
        This explicitly enables the Board's SSH service using your supplied
        Linux password. It changes network exposure; set a strong Linux password
        in official setup first. This app never guesses a default password.
      </p>
      <label
        >Type ENABLE SSH {serial}<input
          autocomplete="off"
          bind:value={enableConfirmation}
        /></label
      >
      <button
        disabled={!password || enableConfirmation !== `ENABLE SSH ${serial}`}
        onclick={() =>
          task(async () => {
            await invoke("admin_enable_ssh", {
              serial,
              username,
              password,
              confirmation: enableConfirmation,
            });
            enableConfirmation = "";
            message =
              "SSH service enabled. Authenticate above to verify your Linux credentials.";
          }, true)}>Enable SSH over this USB connection</button
      >
    </details>
  </fieldset>
  <fieldset
    hidden={area === "setup" || area === "updates"}
    disabled={!native || busy || restorePending || !authenticated}
  >
    <legend>3. Board configuration</legend>
    <button onclick={() => task(() => action({ operation: "status" }))}
      >Refresh Board status</button
    >
    <label>Board hostname<input maxlength="63" bind:value={hostname} /></label>
    <button
      disabled={!hostname || !password}
      onclick={() =>
        task(() => action({ operation: "hostname", hostname, password }), true)}
      >Set hostname using the password above</button
    >
    <button onclick={() => task(() => action({ operation: "wifi_scan" }))}
      >Scan Wi-Fi networks</button
    >
    {#if ssids.length}<label
        >Visible networks<select bind:value={ssid}
          ><option value=""
            >Choose a network or enter a hidden name below</option
          >{#each ssids as name}<option value={name}>{name}</option
            >{/each}</select
        ></label
      >{/if}
    <label
      >Wi-Fi name / hidden SSID<input maxlength="32" bind:value={ssid} /></label
    >
    <label
      >Wi-Fi password (this request only)<input
        type="password"
        autocomplete="off"
        maxlength="128"
        bind:value={wifiPassword}
      /></label
    >
    <button
      disabled={!ssid}
      onclick={() =>
        task(
          () =>
            action({ operation: "wifi_join", ssid, password: wifiPassword }),
          true,
        )}>Join this Wi-Fi network</button
    >
  </fieldset>
  <details hidden={!!area && area !== "setup"}>
    <summary>Prepare initial Companion files and identity</summary>
    <p>
      After official first-time account setup, this prepares the fixed Companion
      source and a private Board-generated TBP TLS identity on a supported
      Debian 13 / arduino UID-1000 baseline. Transfers resume by verified offset
      and publish only into an absent reserved app directory. Existing or
      different apps, credentials and keys are never overwritten or migrated.
    </p>
    <p>
      Source release: <span class="digest"
        >{initialRelease || "native app required"}</span
      >. No Arduino CLI, app start, MCU upload, OS flash or reboot is run by
      this preparation button.
    </p>
    <fieldset
      disabled={!native || busy || restorePending || !board || !usbConfirmed}
    >
      <button
        onclick={() =>
          task(async () => {
            const result = await invoke<{
              phase: string;
              offset: number;
              correctionPhase?: string;
            }>("inspect_initial_companion", { serial });
            initialStatus = `${result.phase}; ${result.offset} bytes transferred${result.correctionPhase ? `; source correction ${result.correctionPhase}` : ""}`;
          })}>Inspect initial preparation</button
      >
      <label
        >Type PREPARE {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={prepareConfirmation}
        /></label
      >
      <button
        disabled={prepareConfirmation !== `PREPARE ${board?.serial}`}
        onclick={async () => {
          const confirmation = prepareConfirmation;
          prepareConfirmation = "";
          await task(async () => {
            const result = await invoke<{ phase: string; message: string }>(
              "prepare_initial_companion",
              { serial, confirmation },
            );
            initialStatus = result.phase;
            message = result.message;
          }, true);
        }}>Prepare / resume initial Companion</button
      >
      <p>
        Narrow #29 correction only: the exact owned, never-started preparation
        can switch to the compile-validated pinned profile while preserving its
        UUID, private TLS key, certificate and configuration. A private copy of
        the original is retained for recovery. Unknown, changed, cached or
        previously started apps are refused. Close official App Lab and do not
        start/build/stop the app concurrently. This does not upload or start it.
      </p>
      <label
        >Type CORRECT PREPARED {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={correctPreparedConfirmation}
        /></label
      >
      <button
        disabled={correctPreparedConfirmation !==
          `CORRECT PREPARED ${board?.serial}`}
        onclick={async () => {
          const confirmation = correctPreparedConfirmation;
          correctPreparedConfirmation = "";
          await task(async () => {
            const result = await invoke<{
              correctionPhase: string;
              message: string;
            }>("correct_prepared_companion", { serial, confirmation });
            initialStatus = result.correctionPhase;
            message = result.message;
          }, true);
        }}>Correct / resume owned unstarted profile (keeps identity)</button
      >
      <label
        >Type RESTORE PREPARED {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={restorePreparedConfirmation}
        /></label
      >
      <button
        disabled={restorePreparedConfirmation !==
          `RESTORE PREPARED ${board?.serial}`}
        onclick={async () => {
          const confirmation = restorePreparedConfirmation;
          restorePreparedConfirmation = "";
          await task(async () => {
            const result = await invoke<{
              correctionPhase: string;
              message: string;
            }>("restore_prepared_companion", { serial, confirmation });
            initialStatus = result.correctionPhase;
            message = result.message;
          }, true);
        }}>Restore original unstarted files (not safe for first start)</button
      >
      <label
        >Type ABORT PREPARE {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={cancelPrepareConfirmation}
        /></label
      >
      <button
        disabled={cancelPrepareConfirmation !==
          `ABORT PREPARE ${board?.serial}`}
        onclick={async () => {
          const confirmation = cancelPrepareConfirmation;
          cancelPrepareConfirmation = "";
          await task(async () => {
            const result = await invoke<{ phase: string; message: string }>(
              "cancel_initial_companion",
              { serial, confirmation },
            );
            initialStatus = result.phase;
            message = result.message;
          }, true);
        }}>Cancel partial initial transfer (keeps published files)</button
      >
    </fieldset>
    {#if initialStatus}<p role="status">
        Initial preparation: {initialStatus}. Not proof of a running app or
        completed setup.
      </p>{/if}
    <fieldset
      disabled={!native ||
        busy ||
        restorePending ||
        !board ||
        !usbConfirmed ||
        !pairedBoardId}
    >
      <p>
        For this exact registered, prepared baseline, certificate renewal
        preserves its key and registrations. The supervisor restarts only its
        Linux child; verification uses the prior TLS anchor before refreshing
        the saved certificate. Pending runtime work, unknown baselines or
        changed keys are refused.
      </p>
      <label
        >Type RENEW TLS {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={renewIdentityConfirmation}
        /></label
      >
      <button
        disabled={renewIdentityConfirmation !== `RENEW TLS ${board?.serial}`}
        onclick={async () => {
          const confirmation = renewIdentityConfirmation;
          renewIdentityConfirmation = "";
          await task(async () => {
            const result = await invoke<{ message: string }>(
              "renew_companion_identity",
              { serial, boardId: pairedBoardId, confirmation },
            );
            message = result.message;
          }, true);
        }}>Renew prepared Board certificate (same key)</button
      >
    </fieldset>
    <p>
      Next, explicitly start this app in official Arduino App Lab. Start/build
      can download dependencies, upload the STM32 sketch and stop another app:
      review the selected target, back up needed data and disconnect unsafe
      loads first. Tinkeroo does not perform that firmware-affecting step
      automatically. Return here for installation Pairing after the app is
      actually running; preparation alone cannot complete the guide.
    </p>
  </details>
  <details hidden={!!area && area !== "setup"}>
    <summary>Installation Pairing and lost-Mac revocation</summary>
    <p>
      This requires an updated, already installed Companion explicitly
      configured for TBP authentication, its host management helper, and a
      supported unprivileged USB account. It does not install/upgrade the
      Companion or finish first-run setup. Unsupported images/accounts fail
      closed.
    </p>
    <p>
      Pairing learns the TLS certificate directly over physical USB, registers
      separate gameplay/SSH public keys, and proves the gameplay private key to
      obtain short-lived sessions. Private keys stay in macOS Keychain. SSH
      login, certificate renewal, deployment and physical acceptance remain
      separate work; the setup guide must not claim completion.
    </p>
    <fieldset
      disabled={!native || busy || restorePending || !board || !usbConfirmed}
    >
      <label
        >Mac label for recovery<input
          autocomplete="off"
          maxlength="60"
          bind:value={installationLabel}
        /></label
      >
      <label
        >Type PAIR {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={pairConfirmation}
        /></label
      >
      <button
        disabled={!installationLabel ||
          pairConfirmation !== `PAIR ${board?.serial}`}
        onclick={() =>
          task(async () => {
            await invoke("pair_installed_companion", {
              serial,
              confirmation: pairConfirmation,
              installationLabel,
            });
            pairConfirmation = "";
            await onForgot();
            message =
              "Installation keys, restricted dedicated-key SSH and signed pinned-TLS authentication were verified for the same Board/kernel. Select the paired Board and Connect explicitly. This alone did not deploy the Companion or complete later runtime/restart/hardware/Crew checkpoints or physical acceptance.";
          }, true)}>Register this Mac with the installed TBP Companion</button
      >
      <button
        onclick={() =>
          task(async () => {
            const result = await invoke<{
              serial: string;
              pairings: {
                installation_id: string;
                label: string;
                state: string;
              }[];
            }>("list_board_pairings", { serial });
            pairingSerial = result.serial;
            pairings = result.pairings;
            revokeInstallation = "";
            revokeConfirmation = "";
          })}>List installations over physical USB</button
      >
      <label
        >Installation to revoke<select bind:value={revokeInstallation}
          ><option value="">Choose explicitly</option
          >{#each pairings as pairing}<option value={pairing.installation_id}
              >{pairing.label} · {pairing.installation_id} · {pairing.state}</option
            >{/each}</select
        ></label
      >
      <p>
        This revokes the selected installation on the physical Board, including
        its active gameplay sessions. Only its tagged SSH key is removed;
        unrelated keys and Mac Mission progress remain. This is different from
        local record cleanup below.
      </p>
      <label
        >Type REVOKE {revokeInstallation || "installation-id"} ON {pairingSerial ||
          "hardware-serial"}<input
          autocomplete="off"
          bind:value={revokeConfirmation}
        /></label
      >
      <button
        disabled={!revokeInstallation ||
          !pairingSerial ||
          revokeConfirmation !==
            `REVOKE ${revokeInstallation} ON ${pairingSerial}`}
        onclick={() =>
          task(async () => {
            const result = await invoke<{ ssh_key_removed: boolean }>(
              "revoke_board_pairing",
              {
                serial,
                installationId: revokeInstallation,
                confirmation: revokeConfirmation,
              },
            );
            revokeConfirmation = "";
            pairings = [];
            revokeInstallation = "";
            message = result.ssh_key_removed
              ? "The Board revoked the installation, invalidated its sessions, and removed only its tagged SSH key."
              : "The Board revoked the installation and invalidated its sessions. SSH file cleanup remains pending; the leftover forced key is disabled by the registry. Resolve file permissions through official management and retry cleanup.";
          }, true)}>Revoke selected installation on this Board</button
      >
    </fieldset>
  </details>
  <details hidden={!!area && area !== "updates"}>
    <summary>Recoverable Linux runtime update</summary>
    <p>
      Updates only the six bundled Companion runtime modules on an already
      installed, compatible TBP baseline with its supervisor running. Firmware,
      dependencies, bootstrap helpers, credentials, and Pairings cannot be
      changed by this path. First installation and firmware/dependency changes
      still require separate supported setup.
    </p>
    <p>
      Arduino App CLI restart can re-upload the sketch; this updater
      deliberately does not call it. A local supervisor restarts only its own
      Python child. Unacknowledged or crashing trials roll back without this Mac
      staying connected. A fresh authenticated, disarmed health check is
      required before committing.
    </p>
    <p>
      Bundled release: <span class="digest"
        >{runtimeRelease || "native app required"}</span
      >
    </p>
    <p>
      Select the exact registered Board in the Companion connection selector
      below first. No update automatically reconnects or rearms gameplay.
    </p>
    <fieldset
      disabled={!native ||
        busy ||
        restorePending ||
        !board ||
        !usbConfirmed ||
        !pairedBoardId}
    >
      <button
        onclick={() =>
          task(async () => {
            const status = await invoke<{
              phase: string;
              current: string;
              offset: number;
              supervisor_ready: boolean;
            }>("runtime_update_status", { serial });
            runtimeStatus = `${status.phase}; ${status.offset} bytes transferred; supervisor ${status.supervisor_ready ? "present" : "unavailable"}; active ${status.current}`;
          })}>Inspect runtime update status over USB</button
      >
      <label
        >Type UPDATE {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={runtimeConfirmation}
        /></label
      >
      <button
        disabled={runtimeConfirmation !== `UPDATE ${board?.serial}`}
        onclick={async () => {
          const confirmation = runtimeConfirmation;
          runtimeConfirmation = "";
          await task(async () => {
            const result = await invoke<{ phase: string; message: string }>(
              "update_companion_runtime",
              { serial, boardId: pairedBoardId, confirmation },
            );
            runtimeStatus = result.phase;
            message = result.message;
          }, true);
        }}>Update / resume bundled Linux runtime</button
      >
      <p>
        If an earlier transfer/trial is pending after interruption or an app
        upgrade, inspect the same physical Board and resolve it explicitly.
        Abort does not undo an already committed update or erase Board data.
      </p>
      <label
        >Type ABORT UPDATE {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={runtimeAbortConfirmation}
        /></label
      >
      <button
        disabled={runtimeAbortConfirmation !== `ABORT UPDATE ${board?.serial}`}
        onclick={async () => {
          const confirmation = runtimeAbortConfirmation;
          runtimeAbortConfirmation = "";
          await task(async () => {
            const result = await invoke<{ phase: string; message: string }>(
              "abort_companion_runtime_update",
              { serial, boardId: pairedBoardId, confirmation },
            );
            runtimeStatus = result.phase;
            message = result.message;
          }, true);
        }}>Abort / recover pending runtime update</button
      >
    </fieldset>
    {#if runtimeStatus}<p role="status">
        Runtime update: {runtimeStatus}. Physical hardware acceptance is still
        separate.
      </p>{/if}
  </details>
  <details
    class="recovery"
    hidden={!!area && area !== "troubleshooting" && !restorePending}
    bind:open={recoveryOpen}
  >
    <summary>Restart, pairing cleanup, and factory recovery</summary>
    <h3>Restart the UNO Q — keeps its data</h3>
    <p>
      Requires a verified Linux management session and its password. Requests a
      clean reboot in about one minute. It does not factory-reset the Board.
    </p>
    <fieldset disabled={!native || busy || restorePending || !authenticated}>
      <label
        >Type RESTART {board?.serial ?? "hardware-serial"}<input
          autocomplete="off"
          bind:value={restartConfirmation}
        /></label
      >
      <button
        disabled={!password ||
          restartConfirmation !== `RESTART ${board?.serial}`}
        onclick={() =>
          task(
            () =>
              action({
                operation: "reboot",
                password,
                confirmation: restartConfirmation,
              }),
            true,
          )}>Schedule Board restart</button
      >
      <label
        >Linux password for a tracked restart<input
          type="password"
          autocomplete="off"
          bind:value={password}
        /></label
      >
      <button
        disabled={!password ||
          !pairedBoardId ||
          restartConfirmation !== `RESTART ${board?.serial}`}
        onclick={() =>
          task(async () => {
            const result = await invoke<{ state: string }>(
              "request_verified_board_restart",
              {
                serial,
                boardId: pairedBoardId,
                password,
                confirmation: restartConfirmation,
              },
            );
            authenticated = false;
            message = `${result.state}: intent saved before scheduling. This is not proof of a completed reboot; wait and verify the new kernel over LAN and USB.`;
          }, true)}>Schedule tracked paired-Board restart</button
      >
    </fieldset>
    <fieldset
      disabled={!native ||
        busy ||
        restorePending ||
        !board ||
        !usbConfirmed ||
        !pairedBoardId}
    >
      <p>
        For a tracked restart, wait for the Board to return, reinspect its exact
        USB identity, and enter its current LAN IP/hostname. A lost transport or
        an old image is not restart evidence. Unknown scheduling outcomes block
        duplicate tracked requests; use explicit official recovery if needed,
        never factory erase merely to clear this state.
      </p>
      <label
        >Board LAN address after restart<input
          autocomplete="off"
          bind:value={restartLanHost}
        /></label
      >
      <label class="check"
        ><input type="checkbox" bind:checked={restartLocalConsent} />Allow the
        app to contact this explicit local-network endpoint for restart
        verification</label
      >
      <button
        disabled={!restartLanHost.trim() || !restartLocalConsent}
        onclick={() =>
          task(async () => {
            const result = await invoke<{ message: string }>(
              "verify_board_restart",
              {
                serial,
                boardId: pairedBoardId,
                lanHost: restartLanHost.trim(),
                lanPort: 7443,
                localConsent: restartLocalConsent,
              },
            );
            message = result.message;
          }, true)}>Verify new kernel and authenticated LAN + USB</button
      >
    </fieldset>
    <h3>Forget Tinkeroo records — this Mac only</h3>
    <p>
      Removes this Mac's saved gameplay credential, not Board apps, Wi-Fi, Linux
      credentials, another installation, or remote authorized_keys. Local
      cleanup does not revoke credentials on the Board. Use physical
      installation revocation above for TBP Pairings; copied legacy bearer
      tokens require explicit Board-side credential rotation. Mission progress
      is retained.
    </p>
    <fieldset disabled={!native || busy || restorePending || !pairedBoardId}>
      <label
        >Type FORGET {pairedBoardId || "selected-board-id"}<input
          autocomplete="off"
          bind:value={forgetConfirmation}
        /></label
      >
      <button
        disabled={forgetConfirmation !== `FORGET ${pairedBoardId}`}
        onclick={() =>
          task(async () => {
            await invoke("forget_board", {
              boardId: pairedBoardId,
              confirmation: forgetConfirmation,
            });
            forgetConfirmation = "";
            await onForgot();
            message =
              "This Mac's gameplay record was removed. Board data and Mission progress were not erased.";
          }, true)}>Forget selected Tinkeroo pairing on this Mac</button
      >
    </fieldset>
    <p>
      After an intentional factory restore, the Board's SSH key changes. Clear
      only this app's SSH fingerprint after physical USB inspection; never
      accept an unexplained key change.
    </p>
    <fieldset disabled={!native || busy || restorePending || !serial}>
      <label
        >Type FORGET SSH {serial || "usb-serial"}<input
          autocomplete="off"
          bind:value={trustConfirmation}
        /></label
      >
      <button
        disabled={trustConfirmation !== `FORGET SSH ${serial}`}
        onclick={() =>
          task(async () => {
            await invoke("admin_forget_ssh_trust", {
              serial,
              confirmation: trustConfirmation,
            });
            trustConfirmation = "";
            authenticated = false;
            message =
              "Only this app's SSH fingerprint was removed after physical inspection. Authenticate again to establish new USB-verified trust.";
          }, true)}>Clear this Mac's SSH trust for the inspected Board</button
      >
    </fieldset>
    <div class="danger-zone">
      <h3>Factory restore — erases the UNO Q Linux system and data</h3>
      <p>
        <strong
          >This is a destructive OS reflash, not a restart or local checkpoint
          reset.</strong
        > Apps, Linux credentials, Wi-Fi configuration, keys, and Board-side Companion
        data are lost. Mac Mission progress is not deleted. Back up/export Board files
        first. This is not a forensic secure-erase guarantee. STM32 sketch/bootloader
        recovery is not verified here; keep maker loads disconnected until known Companion
        firmware and its hardware self-test are installed and checked.
      </p>
      <p>
        Requires the separately installed official <strong
          >Arduino Flasher CLI 0.5.4</strong
        >, internet for the exact approved image, at least 10 GiB free disk
        space, and physical EDL preparation. No Google/Arduino flasher binary is
        bundled or downloaded automatically. Physical restore acceptance is
        still pending; do not try this casually with valuable data.
      </p>
      <p>
        <button onclick={() => openResource("recovery")}
          >Official UNO Q EDL/restore instructions</button
        ><button onclick={() => openResource("flasher_download")}
          >Official flasher download</button
        >
      </p>
      <fieldset disabled={!native || busy || restorePending}>
        <label
          >Absolute path to official arduino-flasher-cli<input
            autocomplete="off"
            placeholder="/Users/you/tools/arduino-flasher-cli"
            bind:value={tool}
            onchange={() => {
              images = [];
              imageIndex = "";
              plan = null;
            }}
          /></label
        >
        <button
          disabled={!tool}
          onclick={() =>
            task(async () => {
              images = await invoke("recovery_images", { tool });
              imageIndex = "";
              plan = null;
            })}>Read supported official UNO Q image catalog</button
        >
        <label
          >Exact official image version<select
            bind:value={imageIndex}
            onchange={() => {
              plan = null;
            }}
            ><option value=""
              >Choose explicitly — not an implicit latest image</option
            >{#each images as image, index}<option value={String(index)}
                >{image.version} — {image.os}</option
              >{/each}</select
          ></label
        >
        <button
          disabled={!serial || !board || imageIndex === "" || !chosenImage}
          onclick={() =>
            task(async () => {
              plan = await invoke("prepare_factory_restore", {
                serial,
                tool,
                version: chosenImage.version,
                os: chosenImage.os,
              });
              eraseConfirmation = "";
              backup = false;
              edl = false;
              onlyTarget = false;
              power = false;
            }, true)}>Prepare exact-Board restore plan (does not erase)</button
        >
      </fieldset>
      {#if plan}
        <p>
          Target: USB {plan.board.adbSerial},
          <strong>hardware serial {plan.board.serial}</strong>
          · image {plan.image.version}
        </p>
        <p class="digest">Image SHA-256: {plan.image.sha256}</p>
        <p>
          Prepare the plan while the Board is still booted. Then disconnect all
          power, follow Arduino's UNO Q JCTL GND/USB_BOOT jumper instructions,
          and reconnect only that Board over a direct USB data cable. Never move
          the jumper while powered. The explicit hardware serial—not whichever
          EDL device appears first—is passed to the flasher. Plans expire after
          ten minutes; preparation itself never erases anything.
        </p>
        <fieldset disabled={!native || busy || restorePending}>
          <label class="check"
            ><input type="checkbox" bind:checked={backup} />I backed up needed
            Board files and understand that the Board's Linux system and data
            will be erased</label
          >
          <label class="check"
            ><input type="checkbox" bind:checked={edl} />I followed the official
            UNO Q EDL preparation with power disconnected</label
          >
          <label class="check"
            ><input type="checkbox" bind:checked={onlyTarget} />Only the exact
            target Board is connected; other Boards are unplugged</label
          >
          <label class="check"
            ><input type="checkbox" bind:checked={power} />Loads/accessories are
            disconnected and the Board has stable power and a good direct data
            cable</label
          >
          <label
            >Type ERASE {plan.board.serial}<input
              autocomplete="off"
              bind:value={eraseConfirmation}
            /></label
          >
          <button
            class="destructive"
            disabled={!canErase}
            onclick={() =>
              task(async () => {
                restore = await invoke("start_factory_restore", {
                  planId: plan!.planId,
                  confirmation: {
                    phrase: eraseConfirmation,
                    backup,
                    edl,
                    onlyTarget,
                    power,
                  },
                });
                authenticated = false;
                eraseConfirmation = "";
                backup = false;
                edl = false;
                onlyTarget = false;
                power = false;
              }, true)}>Erase this UNO Q and restore the approved image</button
          >
        </fieldset>
      {/if}
      <p role="status">
        <strong>Recovery: {restore.phase}</strong> — {restore.message}{restore.bytes
          ? ` (${Math.round(restore.bytes / 1024 / 1024)} MiB downloaded)`
          : ""}
      </p>
      {#if restorePending}<p class="notice">
          Do not close the app, unplug, cancel, or start another flasher once
          writing starts. The native flasher continues if the app closes; a
          previous unknown outcome blocks a new automatic erase.
        </p>{/if}
      {#if ["awaiting_boot", "outcome_unknown", "flash_failed", "download_interrupted"].includes(restore.phase)}
        <fieldset disabled={!native || busy}>
          <p>
            After confirmed flasher completion: unplug, remove the EDL jumper,
            reconnect, and wait for boot. Verification requires the same
            hardware serial and exact approved image build (the USB identifier
            can change). An unknown/failed flasher outcome is not accepted as
            success merely because an old image boots. An interrupted download
            that never started flashing can be discarded after identity
            verification, without claiming a restore.
          </p>
          <button
            onclick={() =>
              task(async () => {
                devices = await invoke("inspect_setup_usb");
              })}>Refresh USB Boards for recovery verification</button
          >
          <label
            >USB serial to verify<select bind:value={serial}
              ><option value="">Choose the reconnected Board explicitly</option
              >{#each devices as device}<option value={device.serial}
                  >{device.serial} — {device.state}</option
                >{/each}</select
            ></label
          >
          {#if ["outcome_unknown", "flash_failed"].includes(restore.phase)}
            <label class="check"
              ><input
                type="checkbox"
                bind:checked={officialRecoveryConfirmed}
                onchange={() => {
                  verifyConfirmation = "";
                }}
              />The official Arduino tool reported a successful destructive
              restore of this exact Board with user-data preservation disabled,
              and no flasher is still running. Record this as my confirmation,
              not an app-observed restore.</label
            >
            <p>
              If an interrupted process has no recorded PID, ownership remains
              uncertain on this Mac boot. Never restart the Mac during active
              flashing; after all flashing has stopped, a Mac restart and
              successful official recovery allow this explicit confirmation.
            </p>
          {/if}
          <label
            >Type {officialRecoveryConfirmed ? "RECOVERED" : "VERIFY"} followed by
            the hardware serial<input
              autocomplete="off"
              bind:value={verifyConfirmation}
            /></label
          >
          <button
            disabled={!serial ||
              !verifyConfirmation ||
              (["outcome_unknown", "flash_failed"].includes(restore.phase) &&
                !officialRecoveryConfirmed)}
            onclick={() =>
              task(async () => {
                const verified = await invoke<{
                  board: Identity;
                  source:
                    | "tinkeroo_flasher"
                    | "human_official_recovery"
                    | "no_flash_started";
                }>("verify_factory_boot", {
                  serial,
                  confirmation: verifyConfirmation,
                  officialRecoveryConfirmed,
                });
                board = verified.board;
                plan = null;
                officialRecoveryConfirmed = false;
                verifyConfirmation = "";
                restore = await invoke("factory_restore_status");
                message =
                  verified.source === "no_flash_started"
                    ? "The same physical Board was verified and the interrupted download/plan was discarded. This app never started flashing and no factory restore is claimed. Prepare a fresh plan if you still want to erase."
                    : `${verified.source === "human_official_recovery" ? "Official-tool restore completion was confirmed by you, not observed by this app." : "This app observed flasher success."} The same Board and exact approved image build were verified. Reconfigure Linux/Wi-Fi, clear old SSH trust explicitly, and establish new Companion Pairing.`;
              })}>Verify Board boot and recovery evidence</button
          >
        </fieldset>
      {/if}
    </div>
  </details>
</section>

<style>
  .management {
    margin: 26px 0;
    padding: 24px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 16px;
  }
  h3 {
    font-size: 22px;
  }
  fieldset {
    margin: 18px 0;
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: 12px;
  }
  label {
    display: block;
    margin: 12px 0;
  }
  input,
  select {
    width: 100%;
    display: block;
    font: inherit;
    padding: 10px;
    margin-top: 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .check {
    display: flex;
    gap: 12px;
    align-items: start;
  }
  .check input {
    width: 22px;
    margin-top: 2px;
  }
  button {
    font: inherit;
    padding: 10px 15px;
    margin: 6px 10px 6px 0;
    border: 1px solid var(--border);
    border-radius: 9px;
    color: var(--primary);
    background: var(--surface);
  }
  .primary {
    color: white;
    background: var(--primary);
  }
  .destructive {
    color: white;
    background: var(--danger);
  }
  button:disabled {
    opacity: 0.5;
  }
  .error {
    padding: 14px;
    border-radius: 10px;
    color: var(--danger);
    background: var(--danger-bg);
  }
  .notice {
    padding: 14px;
    border-radius: 10px;
    color: var(--warning-ink);
    background: var(--warning-bg);
  }
  .recovery {
    margin-top: 24px;
  }
  .danger-zone {
    margin-top: 28px;
    padding: 22px;
    border: 2px solid var(--danger);
    border-radius: 14px;
  }
  .digest {
    overflow-wrap: anywhere;
    font:
      12px ui-monospace,
      monospace;
  }
</style>
