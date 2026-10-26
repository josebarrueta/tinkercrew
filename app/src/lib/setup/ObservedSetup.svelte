<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { errorMessage } from "../errors";
  import { MissionSession } from "../missions/session";
  import { ProgressRepository } from "../missions/storage";
  import { treasureGuardian } from "../missions/catalog";
  import { tracks, type Track } from "../missions/definition";

  export type Observation = {
    serial: string;
    soc: string;
    boardId: string;
    bootId: string;
    runtimeRelease: string;
    runtimeNonce: string;
    hardware: unknown;
  };
  let {
    stepId,
    serial,
    observation,
    beforeChange,
    onRefresh,
    onBegin,
  }: {
    stepId: string;
    serial: string;
    observation: Observation | null;
    beforeChange: () => Promise<void>;
    onRefresh: () => Promise<void>;
    onBegin: () => void;
  } = $props();
  const native = isTauri();
  let disposed = false;
  let busy = $state(false),
    error = $state(""),
    message = $state("");
  let currentSerial = $state(""),
    soc = $state(""),
    physical = $state(false);
  let label = $state("Family Mac"),
    confirmation = $state(""),
    password = $state("");
  let lanHost = $state(""),
    consent = $state(false);
  let running = $state(false),
    polling = false;
  let buttons = $state([false, false, false]),
    moved = $state(false),
    distance = $state<number | null>(null);
  let lights = $state(false),
    sound = $state(false),
    crewConfirmed = $state(false);
  let crewNames = $state(["", "", ""]);
  let crewTracks = $state<Track[]>(["Explorer", "Inventor", "Hacker"]);
  let crewRoles = $state(["Storykeeper", "Builder", "Logic keeper"]);
  let crewLoaded = $state(false);
  let crewSession = $state<MissionSession | undefined>();
  const repository = new ProgressRepository();
  const boardId = $derived(observation?.boardId ?? "");
  $effect(() => {
    if (
      !currentSerial ||
      (serial !== currentSerial && stepId !== "rediscover")
    ) {
      currentSerial = serial;
      soc = "";
      physical = false;
    }
  });
  $effect(() => {
    if (stepId === "crew" && native && !crewLoaded) {
      crewLoaded = true;
      void repository
        .load(treasureGuardian.id)
        .then((saved) => {
          crewSession = new MissionSession(treasureGuardian, saved);
          const crew = crewSession.getProgress().crew;
          crewNames = crew.map((member) => member.name);
          crewTracks = crew.map((member) => member.track);
          crewRoles = crew.map((member) => member.role);
        })
        .catch((cause) => {
          error = `Saved Crew/progress could not be opened; it was not reset: ${errorMessage(cause)}`;
        });
    }
  });
  async function task(action: () => Promise<void>, changing = true) {
    if (!native || busy) return;
    busy = true;
    error = "";
    try {
      if (changing) await beforeChange();
      if (disposed) return;
      await action();
      if (!disposed) await onRefresh();
    } catch (cause) {
      error = errorMessage(
        cause,
        "Native proof failed; no completion was assumed.",
      );
    } finally {
      password = "";
      busy = false;
    }
  }
  async function beginHardware() {
    await task(async () => {
      lights = false;
      sound = false;
      buttons = [false, false, false];
      moved = false;
      const result = await invoke<{ message: string }>("start_setup_hardware", {
        boardId,
      });
      if (disposed) {
        await invoke("cancel_setup_hardware");
        return;
      }
      running = true;
      crewLoaded = false;
      message = result.message;
    });
  }
  onMount(() => {
    const timer = setInterval(() => {
      if (!native || !running || busy || polling) return;
      polling = true;
      void invoke<{
        buttons: boolean[];
        distanceMoved: boolean;
        distanceMillimeters: number;
      }>("poll_setup_hardware", { boardId })
        .then((view) => {
          buttons = view.buttons;
          moved = view.distanceMoved;
          distance = view.distanceMillimeters;
        })
        .catch((cause) => {
          running = false;
          error = errorMessage(cause);
        })
        .finally(() => {
          polling = false;
        });
    }, 500);
    return () => clearInterval(timer);
  });
  onDestroy(() => {
    disposed = true;
    password = "";
    if (native && running) void invoke("cancel_setup_hardware").catch(() => {});
  });
</script>

<section aria-label="Observed setup proofs">
  <p>
    These steps advance only when native code records the required proof. A
    frontend checkbox, legacy connection import, simulator or scheduled reboot
    cannot substitute. Unknown baselines retain the official-tool handoff.
  </p>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if stepId !== "complete"}
    <fieldset disabled={!native || busy}>
      <label
        >Current setup USB serial<input
          bind:value={currentSerial}
          readonly={stepId !== "rediscover"}
        /></label
      >
      <button
        onclick={() =>
          task(async () => {
            const host = await invoke<{ serial: string }>("admin_inspect_usb", {
              serial: currentSerial,
            });
            soc = host.serial;
            physical = false;
            message = `Observed hardware serial ${soc}. Compare the exact physical target before continuing.`;
          }, false)}>Inspect exact Board for observed setup</button
      >
      <label
        ><input type="checkbox" bind:checked={physical} disabled={!soc} />I
        physically verified this selected setup Board and hardware serial</label
      >
    </fieldset>
  {/if}
  {#if stepId === "pair"}
    <p>
      Prepare the initial files/private identity in Board Setup above and
      explicitly start the app in official App Lab first. Start can upload the
      sketch; review target/backups/loads. Enable SSH explicitly, including for
      verification over USB. No password or key is guessed.
    </p>
    <fieldset disabled={!native || busy || !soc || !physical}>
      <label
        >Recognizable Mac label<input
          maxlength="60"
          bind:value={label}
        /></label
      >
      <label
        >Type PAIR {soc || "hardware-serial"}<input
          autocomplete="off"
          bind:value={confirmation}
        /></label
      >
      <button
        disabled={confirmation !== `PAIR ${soc}` || !label.trim()}
        onclick={() =>
          task(async () => {
            await invoke("pair_installed_companion", {
              serial: currentSerial,
              confirmation,
              installationLabel: label.trim(),
            });
            confirmation = "";
            message =
              "Registration, dedicated-key SSH and signed TLS were verified; reload reads the native checkpoint, not a client claim.";
          })}>Prove installation Pairing</button
      >
    </fieldset>
  {:else if stepId === "deploy"}
    <p>
      Only the compiled compatible Linux runtime is eligible. This path never
      invokes firmware tooling or overwrites an unsupported whole-app baseline.
    </p>
    <fieldset disabled={!native || busy || !boardId || !soc || !physical}>
      <label
        >Type UPDATE {soc || "hardware-serial"}<input
          autocomplete="off"
          bind:value={confirmation}
        /></label
      >
      <button
        disabled={confirmation !== `UPDATE ${soc}`}
        onclick={() =>
          task(async () => {
            const result = await invoke<{ message: string }>(
              "update_companion_runtime",
              { serial: currentSerial, boardId, confirmation },
            );
            confirmation = "";
            message = result.message;
          })}>Verify / update supported runtime</button
      >
    </fieldset>
  {:else if stepId === "rediscover"}
    <p>
      Use the authenticated Linux management controls above first. A tracked
      request records intent before scheduling; unknown outcomes never invite an
      automatic duplicate.
    </p>
    <fieldset disabled={!native || busy || !boardId || !soc || !physical}>
      <label
        >Linux password for this approved setup restart<input
          type="password"
          autocomplete="off"
          bind:value={password}
        /></label
      >
      <label
        >Type RESTART {soc || "hardware-serial"}<input
          autocomplete="off"
          bind:value={confirmation}
        /></label
      >
      <button
        disabled={!password || confirmation !== `RESTART ${soc}`}
        onclick={() =>
          task(async () => {
            await invoke("admin_login", {
              serial: currentSerial,
              username: "arduino",
              password,
              usbConfirmed: physical,
            });
            if (disposed) {
              await invoke("admin_logout");
              return;
            }
            await invoke("request_verified_board_restart", {
              serial: currentSerial,
              boardId,
              password,
              confirmation,
            });
            confirmation = "";
            message =
              "Restart requested/scheduled; completion is still pending a new kernel and authenticated LAN plus USB.";
          })}>Request tracked setup restart</button
      >
      <label
        >Current Board LAN address<input
          autocomplete="off"
          bind:value={lanHost}
        /></label
      >
      <label
        ><input type="checkbox" bind:checked={consent} />Allow contact with this
        explicit local-network endpoint</label
      >
      <button
        disabled={!lanHost.trim() || !consent}
        onclick={() =>
          task(async () => {
            const result = await invoke<{ message: string }>(
              "verify_board_restart",
              {
                serial: currentSerial,
                boardId,
                lanHost: lanHost.trim(),
                lanPort: 7443,
                localConsent: consent,
              },
            );
            message = result.message;
          })}>Observe new kernel over LAN and USB</button
      >
    </fieldset>
  {:else if stepId === "hardware"}
    <p>
      Self-test outputs are bounded. Move Distance by at least 10 mm and press
      A, B and C after starting. Keep live inputs fresh; human light/sound
      confirmation is not an electrical safe-off measurement.
    </p>
    <fieldset disabled={!native || busy || !boardId}>
      <button onclick={beginHardware}>Start bounded setup self-test</button>
      <p>
        Buttons: {buttons
          .map(
            (pressed, index) =>
              `${"ABC"[index]} ${pressed ? "observed" : "pending"}`,
          )
          .join(" · ")}; distance movement: {moved
          ? "observed"
          : "pending"}{distance !== null ? ` (${distance} mm)` : ""}.
      </p>
      <label
        ><input type="checkbox" bind:checked={lights} />We actually saw the
        self-test lights</label
      >
      <label
        ><input type="checkbox" bind:checked={sound} />We actually heard the
        self-test sound</label
      >
      <button
        disabled={!running ||
          !buttons.every(Boolean) ||
          !moved ||
          !lights ||
          !sound}
        onclick={() =>
          task(async () => {
            const result = await invoke<{ message: string }>(
              "finish_setup_hardware",
              { boardId, lights, sound },
            );
            running = false;
            message = result.message;
          })}>Confirm observed setup self-test</button
      >
    </fieldset>
    <p>
      If the kernel changed after verification, use the parent restart
      verification controls above and reload this guide. Only downstream
      hardware evidence is invalidated; keys, networks and Mission progress are
      retained.
    </p>
  {:else if stepId === "crew"}
    <p>
      Keep existing Mission/remix/progress intact. Save three Crew members and
      confirm their roles/tracks. If the hardware witness expires, explicitly
      retest rather than reusing stale evidence.
    </p>
    <fieldset disabled={!native || busy || !boardId || !crewSession}>
      {#each crewRoles as role, index}
        <label
          >{role} name<input
            maxlength="60"
            bind:value={crewNames[index]}
          /></label
        >
        <label
          >{role} track<select bind:value={crewTracks[index]}
            >{#each tracks as track}<option value={track}>{track}</option
              >{/each}</select
          ></label
        >
      {/each}
      <label
        ><input type="checkbox" bind:checked={crewConfirmed} />We confirm this
        saved Crew and are ready to begin</label
      >
      <button
        disabled={!crewConfirmed || crewNames.some((name) => !name.trim())}
        onclick={() =>
          task(async () => {
            if (!crewSession) throw new Error("Load Crew progress first.");
            crewSession.setCrew(
              crewNames.map((name, index) => ({
                name: name.trim(),
                track: crewTracks[index],
              })),
            );
            await repository.save(crewSession.getProgress());
            const result = await invoke<{ message: string }>(
              "finish_setup_crew",
              { boardId, crewConfirmed },
            );
            message = result.message;
          })}>Save Crew and record setup completion</button
      >
      <button onclick={beginHardware}>Repeat fresh hardware proof</button>
    </fieldset>
  {:else if stepId === "complete"}
    <p role="status">
      Supported hybrid setup was recorded. This historical record does not prove
      current wiring or grant arming. Reconnect explicitly and obtain fresh
      Mission hardware evidence; physical/family acceptance remains separate.
    </p>
    <button onclick={onBegin}>Return to the Crew’s Mission choices</button>
  {/if}
  <button disabled={!native || busy} onclick={() => task(onRefresh, false)}
    >Reload native setup proof</button
  >
</section>

<style>
  section {
    display: grid;
    gap: 0.65rem;
  }
  fieldset {
    display: grid;
    gap: 0.6rem;
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 0.8rem;
  }
  label {
    display: grid;
    gap: 0.3rem;
  }
  p {
    margin: 0;
  }
</style>
