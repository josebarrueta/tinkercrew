<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    pushState,
    replaceState,
    goto,
    beforeNavigate,
  } from "$app/navigation";
  import { rememberDraft, takeDraft } from "./return-draft";
  import { isTauri } from "@tauri-apps/api/core";
  import ConnectionPanel from "../board/ConnectionPanel.svelte";
  import { SimulatorBoardClient } from "../board/simulator";
  import type { BoardClient, BoardSnapshot, Capability } from "../board/types";
  import type { ConnectionState } from "../board/connection";
  import { errorMessage } from "../errors";
  import { CreationRuntime } from "./runtime";
  import {
    guardianStarter,
    newRule,
    programSchema,
    type Program,
    type Rule,
    type Library,
  } from "./definition";
  import { CreationRepository } from "./storage";

  const simulator = new SimulatorBoardClient();
  const repository = new CreationRepository();
  let board = $state.raw<BoardClient>(simulator);
  const initialRuntime = new CreationRuntime(simulator);
  let actor = $state.raw(initialRuntime);
  let live = $state(initialRuntime.getStatus());
  let snapshot = $state<BoardSnapshot | null>(null);
  type Screen = "play" | "blocks" | "library" | "missions" | "parent";
  let screen = $state<Screen>("play");
  let createView = $state<"play" | "blocks">("play");
  let returnView: Screen = "play";
  type ParentTask =
    "overview" | "setup" | "network" | "updates" | "troubleshooting";
  let parentTask = $state<ParentTask>("overview");
  let managementBusy = $state(false);
  let connectionSection = $state<HTMLElement>();
  let connectionReturnFocus: HTMLElement | null = null;
  let selectionEpoch = 0;
  let mainElement: HTMLElement;
  let navigationEpoch = 0;
  let headerHeight = $state(80);
  let program = $state<Program>({ rules: [newRule()] });
  let title = $state("My invention");
  let creationId = $state<string | null>(null);
  let library = $state<Library>({ version: 1, creations: [] });
  let libraryReady = $state(false);
  let savedStatus = $state("Opening saved creations…");
  let message = $state("Try a module, then make it do something of your own.");
  let problem = $state("");
  let working = $state(false);
  let saving = $state(false);
  let unsaved = $state(false);
  type Replacement =
    { kind: "new" | "guardian" | "legacy" } | { kind: "open"; id: string };
  let pendingReplacement = $state<Replacement | null>(null);
  let replacing = $state(false);
  let decisionReturnFocus: HTMLElement | null = null;
  let draftSession = 0,
    draftVersion = 0;
  let lastButton = $state("");
  let connectionInitialized = $state(false);
  let connectionOpen = $state(false);
  let connectionState = $state<ConnectionState>({ phase: "simulator" });
  let connectionPanel = $state<{
    openConnection: () => Promise<void>;
    useSimulator: () => Promise<void>;
    cancelPending: () => Promise<void>;
  }>();
  const practice = $derived(board === simulator);
  const displayError = $derived(
    problem ||
      live.error ||
      (!practice && connectionState.phase === "failed"
        ? connectionState.error.message
        : ""),
  );
  beforeNavigate((navigation) => {
    if (managementBusy && navigation.to?.url.pathname !== "/")
      navigation.cancel();
  });
  const has = (name: Capability) =>
    snapshot?.health !== "disconnected" &&
    (snapshot?.capabilities.includes(name) ?? false);
  function health(name: Capability) {
    if (!snapshot)
      return connectionState.phase === "failed"
        ? "Connection unavailable"
        : "Checking…";
    if (has(name)) return practice ? "Practice module" : "Responding";
    const state = snapshot.capabilityHealth?.[name]?.health;
    return state === "stale"
      ? "Reading is stale"
      : state === "disconnected"
        ? "Not connected"
        : "Not available";
  }
  function dirty() {
    unsaved = true;
    draftVersion++;
    savedStatus = "Unsaved changes.";
  }
  async function stop(forceClear = false) {
    const operation = actor.stop(forceClear);
    live = actor.getStatus();
    await operation;
    live = actor.getStatus();
  }
  function locationView(): Screen {
    const value = new URLSearchParams(location.search).get("view");
    return ["play", "blocks", "library", "missions", "parent"].includes(
      value ?? "",
    )
      ? (value as Screen)
      : "play";
  }
  async function showView(next: Screen, history = true) {
    screen = next;
    if (next === "play" || next === "blocks") createView = next;
    const url = new URL(location.href);
    url.searchParams.set("view", next);
    if (history) pushState(url, {});
    await tick();
    mainElement
      ?.querySelector<HTMLElement>("h1")
      ?.focus({ preventScroll: true });
    window.scrollTo({ top: 0, behavior: "instant" });
  }
  async function navigate(next: Screen, history = true) {
    const epoch = ++navigationEpoch;
    if (next === screen) return;
    if (managementBusy || replacing) {
      if (managementBusy)
        problem =
          "Finish the active Parent task before leaving. Stop outputs is still available.";
      const url = new URL(location.href);
      url.searchParams.set("view", screen);
      replaceState(url, {});
      return;
    }
    await connectionPanel?.cancelPending();
    connectionOpen = false;
    await stop();
    if (epoch !== navigationEpoch) return;
    await showView(next, history);
  }
  async function selectBoard(next: BoardClient) {
    const epoch = ++selectionEpoch;
    await stop();
    if (epoch !== selectionEpoch) return;
    snapshot = null;
    problem = "";
    lastButton = "";
    board = next;
  }
  async function connect() {
    if (managementBusy) return;
    connectionReturnFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const inspectOnly = connectionState.phase === "connected";
    if (!inspectOnly) await stop();
    connectionInitialized = true;
    connectionOpen = true;
    await tick();
    connectionSection
      ?.querySelector<HTMLElement>("h2")
      ?.focus({ preventScroll: true });
    if (!inspectOnly) await connectionPanel?.openConnection();
  }
  async function closeConnection() {
    await connectionPanel?.cancelPending();
    connectionOpen = false;
    await tick();
    connectionReturnFocus?.focus({ preventScroll: true });
  }
  async function practiceMode() {
    await stop();
    if (connectionPanel) await connectionPanel.useSimulator();
    else await selectBoard(simulator);
  }
  async function parentSettings(task: ParentTask = "overview") {
    if (managementBusy) return;
    if (screen !== "parent") returnView = screen;
    if (connectionOpen) await closeConnection();
    connectionInitialized = true;
    parentTask = task;
    await navigate("parent");
  }
  async function selectParentTask(task: ParentTask) {
    if (managementBusy) return;
    await connectionPanel?.cancelPending();
    parentTask = task;
    await tick();
    connectionSection?.querySelector<HTMLElement>("h2")?.focus();
  }
  async function perform(action: () => Promise<void>) {
    if (working || pendingReplacement || replacing) return;
    working = true;
    problem = "";
    try {
      const operation = action();
      live = actor.getStatus();
      await operation;
      live = actor.getStatus();
    } catch (cause) {
      problem = errorMessage(cause);
    } finally {
      working = false;
    }
  }
  function change(index: number, patch: Partial<Rule>) {
    void stop();
    program = {
      rules: program.rules.map((rule, i) =>
        i === index ? { ...rule, ...patch } : rule,
      ),
    };
    dirty();
  }
  function addRule(trigger: Rule["trigger"] = "A") {
    if (program.rules.length >= 8) return;
    void stop();
    program = { rules: [...program.rules, { ...newRule(), trigger }] };
    dirty();
  }
  async function run() {
    if (!programSchema.safeParse(program).success) {
      await stop();
      problem =
        "Use 1–8 rules, distances from 2 to 400 cm, a valid light color and at least one output per rule.";
      return;
    }
    await perform(() => actor.start(program));
  }
  async function save(fork = false) {
    if (!libraryReady || saving) return false;
    saving = true;
    const session = draftSession,
      version = draftVersion;
    try {
      if ((fork || !creationId) && library.creations.length >= 24)
        throw new Error(
          "This library holds up to 24 Creations. Open an existing Creation to edit it; no saved work was removed.",
        );
      const id = fork || !creationId ? crypto.randomUUID() : creationId;
      const entry = {
        id,
        title: fork ? `${title.trim().slice(0, 53)} (copy)` : title.trim(),
        program: programSchema.parse(program),
      };
      const next: Library = {
        version: 1,
        creations: library.creations.some((item) => item.id === id)
          ? library.creations.map((item) => (item.id === id ? entry : item))
          : [...library.creations, entry],
      };
      if (fork) await stop();
      await repository.save(next);
      library = next;
      if (session === draftSession) {
        creationId = id;
        if (version === draftVersion) {
          title = entry.title;
          savedStatus = "Saved on this Mac.";
          unsaved = false;
        }
      }
      return session === draftSession && version === draftVersion;
    } catch (cause) {
      problem = errorMessage(
        cause,
        "Save did not finish. Existing creations were not overwritten.",
      );
      if (session === draftSession)
        savedStatus = "Not saved yet. Retry after correcting the message.";
      return false;
    } finally {
      saving = false;
    }
  }
  async function applyReplacement(target: Replacement) {
    await stop();
    if (target.kind === "legacy") {
      await goto("/legacy");
      return;
    }
    if (target.kind === "open") {
      const saved = library.creations.find((item) => item.id === target.id);
      if (!saved) return;
      program = programSchema.parse(saved.program);
      title = saved.title;
      creationId = saved.id;
      unsaved = false;
      savedStatus = "Saved on this Mac.";
    } else {
      program =
        target.kind === "guardian" ? guardianStarter() : { rules: [newRule()] };
      title =
        target.kind === "guardian" ? "My treasure guardian" : "My invention";
      creationId = null;
      dirty();
      if (target.kind === "guardian")
        message =
          "Guardian starter: cross the 20 cm boundary and notice the light. Change a rule to make it yours.";
    }
    draftSession++;
    await showView("blocks");
    problem = "";
  }
  async function requestReplacement(target: Replacement) {
    if (saving || replacing || pendingReplacement || managementBusy) return;
    replacing = true;
    decisionReturnFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    try {
      await stop();
      if (unsaved) {
        pendingReplacement = target;
      } else await applyReplacement(target);
    } finally {
      replacing = false;
    }
    if (pendingReplacement) {
      await tick();
      document.getElementById("keep-editing")?.focus();
    }
  }
  async function keepEditing() {
    if (replacing || saving) return;
    pendingReplacement = null;
    await tick();
    decisionReturnFocus?.focus();
  }
  async function replaceDraft(saveFirst: boolean) {
    const target = pendingReplacement;
    if (!target || replacing || saving) return;
    replacing = true;
    try {
      // A successful write of an older draft version cannot discard edits
      // made while the write was in flight.
      if (saveFirst && !(await save())) return;
      await applyReplacement(target);
      pendingReplacement = null;
    } finally {
      replacing = false;
    }
  }
  const openCreation = (id: string) => requestReplacement({ kind: "open", id });
  const fresh = () => requestReplacement({ kind: "new" });
  const guardian = () => requestReplacement({ kind: "guardian" });
  onMount(() => {
    const returning = takeDraft();
    if (returning) {
      program = returning.program;
      title = returning.title;
      creationId = returning.id;
      unsaved = returning.unsaved;
      createView = returning.view;
      savedStatus = unsaved ? "Unsaved changes." : "Saved on this Mac.";
    }
    screen =
      returning && !new URLSearchParams(location.search).has("view")
        ? returning.view
        : locationView();
    if (screen === "blocks") createView = "blocks";
    if (screen === "parent") connectionInitialized = true;
    let disposed = false;
    void repository
      .load()
      .then((value) => {
        if (!disposed) {
          library = value;
          libraryReady = true;
          if (!unsaved)
            savedStatus = creationId
              ? "Saved on this Mac."
              : "No changes saved yet.";
        }
      })
      .catch(() => {
        problem =
          "Saved creations could not be read or use an unsupported version. Existing work was not replaced. Keep the save and reopen the app before retrying.";
        savedStatus =
          "Saved library unavailable; saving is disabled to protect it.";
      });
    return () => {
      rememberDraft({
        program: $state.snapshot(program),
        title,
        id: creationId,
        unsaved,
        view: createView,
      });
      disposed = true;
    };
  });
  $effect(() => {
    const selected = board;
    const runtime = new CreationRuntime(selected);
    actor = runtime;
    let disposed = false,
      polling = false;
    const unsubscribe = selected.subscribe((event) => {
      if (event.type === "button.changed" && event.pressed) {
        lastButton = `Button ${"ABC"[event.button]} pressed`;
        if (event.button === 2) void runtime.stop(true);
      }
    });
    async function refresh() {
      if (disposed || polling) return;
      polling = true;
      try {
        await runtime.tick();
        const next = await selected.getSnapshot();
        if (!disposed) {
          snapshot = next;
          live = runtime.getStatus();
        }
      } catch (cause) {
        if (!disposed) {
          snapshot = null;
          problem = errorMessage(
            cause,
            "Board unavailable. Reconnect without deleting Pairing.",
          );
          await runtime.stop();
          live = runtime.getStatus();
        }
      } finally {
        polling = false;
      }
    }
    void refresh();
    const timer = setInterval(() => {
      void refresh();
    }, 250);
    return () => {
      disposed = true;
      clearInterval(timer);
      unsubscribe();
      void runtime.stop();
    };
  });
</script>

<svelte:head><title>Tinkeroo — make hardware your own</title></svelte:head>
<svelte:window
  onpopstate={() => {
    void navigate(locationView(), false);
  }}
  onkeydown={(event) => {
    if (event.key === "Escape" && pendingReplacement) {
      event.preventDefault();
      void keepEditing();
    } else if (event.key === "Escape" && connectionOpen) {
      event.preventDefault();
      void closeConnection();
    }
  }}
/>

<header bind:clientHeight={headerHeight}>
  <button
    class="brand"
    aria-label="Tinkeroo"
    onclick={() => navigate(createView)}
    ><img src="/icon.svg" alt="" width="36" height="36" />Tinkeroo</button
  >
  <nav aria-label="Main navigation">
    <button
      class:active={screen === "play" || screen === "blocks"}
      aria-current={screen === "play" || screen === "blocks"
        ? "page"
        : undefined}
      onclick={() => navigate(createView)}>Create</button
    ><button
      class:active={screen === "library"}
      aria-current={screen === "library" ? "page" : undefined}
      onclick={() => navigate("library")}>My creations</button
    >
  </nav>
  <div class="board-dock">
    <span
      >{practice
        ? "Practice — no real Board outputs"
        : connectionState.phase === "connected"
          ? "Real Board · authenticated connection"
          : "Real Board · connection unavailable"}</span
    ><button onclick={connect}
      >{connectionState.phase === "connected"
        ? "Board details"
        : "Connect Board"}</button
    ><button class="stop" aria-label="Stop outputs" onclick={() => stop(true)}
      >■ Stop outputs</button
    ><button disabled={managementBusy} onclick={() => parentSettings()}
      >Parent settings</button
    >
  </div>
</header>
<main bind:this={mainElement} style:--header-height={`${headerHeight}px`}>
  {#if screen === "play" || screen === "blocks"}
    <nav class="creation-views" aria-label="Creation views">
      <button
        class:active={screen === "play"}
        aria-current={screen === "play" ? "page" : undefined}
        onclick={() => navigate("play")}>Try modules</button
      >
      <button
        class:active={screen === "blocks"}
        aria-current={screen === "blocks" ? "page" : undefined}
        onclick={() => navigate("blocks")}>Build</button
      >
      <button onclick={() => navigate("missions")}>Start from a Mission</button>
    </nav>
  {/if}
  {#if pendingReplacement}
    <section class="draft-decision" aria-label="Unsaved Creation">
      <h2>Keep your Creation?</h2>
      <p>
        Save your changes before opening something else. Your program stays
        stopped until you choose Run again.
      </p>
      {#if pendingReplacement.kind === "legacy"}<p>
          Your draft stays in memory for this detour. Save first to keep it
          after closing or reloading the app. Earlier Mission progress stays
          separate.
        </p>{/if}
      <div class="row">
        <button
          id="keep-editing"
          disabled={replacing || saving}
          onclick={keepEditing}>Keep editing</button
        >
        <button
          disabled={replacing || saving}
          onclick={() => replaceDraft(false)}
          >{pendingReplacement.kind === "legacy"
            ? "Continue without saving"
            : "Discard changes"}</button
        >
        <button
          class="primary"
          disabled={replacing || saving || !libraryReady || !title.trim()}
          onclick={() => replaceDraft(true)}>Save and continue</button
        >
      </div>
    </section>
  {/if}
  {#if displayError && !connectionOpen}<p class="error" role="alert">
      {displayError}
    </p>{/if}
  {#if screen === "parent"}<div class="heading">
      <h1 tabindex="-1">Parent settings</h1>
      <p>
        Setup and maintenance are separate from creating. Firmware, restarts,
        credential changes and erasing data require explicit actions below.
      </p>
    </div>
    <button disabled={managementBusy} onclick={() => navigate(returnView)}
      >Return to my Creation</button
    >
    <nav class="parent-tasks" aria-label="Parent tasks">
      {#each [["setup", "Board setup"], ["network", "Network & saved permission"], ["updates", "Software updates"], ["troubleshooting", "Troubleshooting"]] as [task, label]}
        <button
          disabled={managementBusy}
          aria-current={parentTask === task ? "page" : undefined}
          onclick={() => selectParentTask(task as ParentTask)}>{label}</button
        >
      {/each}
    </nav>
    {#if parentTask === "overview"}<p>
        Choose a task. Normal USB play needs none of these settings. Destructive
        recovery stays inside Troubleshooting.
      </p>{/if}
  {/if}
  {#if connectionInitialized}<section
      bind:this={connectionSection}
      class:connection-sheet={connectionOpen}
      aria-label="Board connection"
      hidden={!connectionOpen &&
        (screen !== "parent" || parentTask === "overview")}
    >
      {#if connectionOpen}<button onclick={closeConnection}
          >Close connection panel</button
        >{/if}
      <ConnectionPanel
        bind:this={connectionPanel}
        visible={connectionOpen ||
          (screen === "parent" && parentTask !== "overview")}
        defer
        advanced={!connectionOpen &&
          screen === "parent" &&
          parentTask !== "overview"}
        parentTask={screen === "parent" && !connectionOpen
          ? parentTask
          : undefined}
        onParent={() => parentSettings("setup")}
        onBusyChange={(busy) => {
          managementBusy = busy;
        }}
        continueLabel="Return to my Creation"
        onMission={() => navigate(returnView)}
        onConnected={async (client) => {
          await selectBoard(client);
          if (board !== client) return;
          connectionOpen = false;
          if (screen === "parent") await navigate(returnView);
          else {
            await tick();
            connectionReturnFocus?.focus({ preventScroll: true });
          }
          message =
            "Board connected. Choose a module test or Run explicitly; outputs do not resume automatically.";
        }}
        onSimulator={() => {
          void selectBoard(simulator);
          connectionOpen = false;
        }}
        onState={(state) => {
          connectionState = state;
        }}
      />
    </section>{/if}

  {#snippet preview()}
    <section class="preview" aria-label="Reported outputs">
      <div class="invention">
        <span>• •</span>
        <div
          class="lamp"
          style:background={snapshot?.outputs.pixels?.color ?? "#d9dce9"}
        ></div>
      </div>
      <p>
        Pixels: <strong data-testid="creation-pixels"
          >{snapshot
            ? (snapshot.outputs.pixels?.color ?? "off")
            : "unknown"}</strong
        >
      </p>
      <p>
        Buzzer: {snapshot
          ? snapshot.outputs.buzzer
            ? "short chirp"
            : "off"
          : "unknown"}{practice ? " · visual practice only" : ""}
      </p>
      <p>Program: <strong data-testid="creation-state">{live.state}</strong></p>
      <small
        >{practice
          ? "These are simulated outputs, not a real hardware test."
          : "Reported state is not proof of light or sound. Observe the actual modules."}</small
      >
    </section>
  {/snippet}
  {#snippet buttonInput()}
    {#if practice}<div class="row">
        {#each ["A", "B", "C · Stop"] as label, index}<button
            onclick={() => simulator.controls.pressButton(index)}
            >{label}</button
          >{/each}
      </div>{:else}<p>
        Use physical A/B/C. C is a software Stop, not a power cutoff.
      </p>
      <div class="row">
        {#each ["A", "B", "C"] as label, index}<span
            class:pressed={has("buttons") &&
              snapshot?.observables.buttons[index]}
            >{label}: {has("buttons")
              ? snapshot?.observables.buttons[index]
                ? "pressed"
                : "released"
              : "unknown"}</span
          >{/each}
      </div>{/if}
    <p aria-live="polite">
      {lastButton || "A and B are inputs. C stops outputs."}
    </p>
  {/snippet}
  {#snippet distanceInput()}
    <p class="reading">
      {has("distance") &&
      Number.isFinite(snapshot?.observables.distanceMillimeters)
        ? `${Math.round(snapshot!.observables.distanceMillimeters) / 10} cm`
        : "No fresh reading"}
    </p>
    {#if practice}<label
        >Move a pretend visitor<input
          aria-label="Practice distance (cm)"
          type="range"
          min="2"
          max="100"
          step="0.1"
          value={(snapshot?.observables.distanceMillimeters ?? 600) / 10}
          oninput={(event) =>
            simulator.controls.setDistance(
              Number(event.currentTarget.value) * 10,
            )}
        /></label
      >{:else}<p>Move an object in front of Distance.</p>{/if}
  {/snippet}
  {#snippet moduleCards()}
    <section class="module-panel">
      <div class="panel-heading">
        <h2>Your modules</h2>
        {#if !practice}<button onclick={practiceMode}>Use Practice</button>{/if}
      </div>

      {#if practice}<p class="muted">
          Explore without a Board. Connect Board when you want to control real
          hardware.
        </p>{/if}
      <div class="module-grid">
        <article>
          <h3>◉ Buttons</h3>
          <p class="health">{health("buttons")}</p>
          {@render buttonInput()}
        </article>
        <article>
          <h3>↔ Distance</h3>
          <p class="health">{health("distance")}</p>
          {@render distanceInput()}
        </article>
        <article>
          <h3>✦ Pixels</h3>
          <p class="health">{health("pixels")}</p>
          <p>Try a color briefly (up to 1.5 seconds).</p>
          <div class="row">
            <button
              disabled={working || !has("pixels")}
              onclick={() =>
                perform(() =>
                  actor.preview({
                    operation: "pixels.set",
                    arguments: { color: "#4986ff", brightness: 0.25 },
                  }),
                )}>Try blue</button
            ><button
              disabled={working || !has("pixels")}
              onclick={() =>
                perform(() =>
                  actor.preview({
                    operation: "pixels.set",
                    arguments: { color: "#a76be8", brightness: 0.25 },
                  }),
                )}>Try purple</button
            >
          </div>
        </article>
        <article>
          <h3>♫ Buzzer</h3>
          <p class="health">{health("buzzer")}</p>
          <p>
            {practice
              ? "Silent visual practice."
              : "This will make a brief sound."} A 250 ms chirp.
          </p>
          <button
            disabled={working || !has("buzzer")}
            onclick={() =>
              perform(() =>
                actor.preview({
                  operation: "buzzer.play",
                  arguments: { frequencyHz: 880, durationMs: 250 },
                }),
              )}>Try chirp</button
          >
        </article>
      </div>
      {#if practice}<details>
          <summary>Practice a stale-sensor situation</summary><label
            class="check"
            ><input
              type="checkbox"
              onchange={(event) =>
                simulator.controls.freezeDistance(event.currentTarget.checked)}
            />Freeze Distance readings</label
          >
          <p>
            A button/light rule can still work without fresh Distance. Rules
            depending on Distance must stop.
          </p>
        </details>{/if}
    </section>
  {/snippet}

  {#if screen === "play"}
    <section class="heading">
      <span class="eyebrow">SMALL EXPERIMENTS. BIG IDEAS.</span>
      <h1 tabindex="-1">First, make something happen.</h1>
      <p>
        No forms. Try a module, notice its response, then make your own
        behavior.
      </p>
    </section>
    <div class="playground">{@render moduleCards()}{@render preview()}</div>
    <section class="next-step">
      <div>
        <span class="eyebrow">GIVE IT A BEHAVIOR</span>
        <h2>What should happen when…?</h2>
        <p>Connect inputs, conditions and outputs with blocks.</p>
      </div>
      <button
        class="primary"
        aria-label="Build with blocks"
        onclick={() => navigate("blocks")}>Build with blocks →</button
      >
    </section>
  {:else if screen === "blocks"}
    <section class="heading">
      <h1 tabindex="-1">Build a behavior, one block at a time.</h1>
    </section>
    <section class="editor-toolbar" aria-label="Creation controls">
      <label
        >Creation name<input
          maxlength="60"
          bind:value={title}
          oninput={dirty}
        /></label
      >
      <div class="row">
        <button
          class="primary"
          disabled={working ||
            !!pendingReplacement ||
            replacing ||
            live.state === "running"}
          aria-label="Run creation"
          onclick={run}>▶ Run creation</button
        >
        <button
          disabled={!libraryReady ||
            saving ||
            live.state === "stopping" ||
            !title.trim()}
          aria-busy={saving}
          onclick={() => save()}>Save creation</button
        >
      </div>
      <p role="status">{savedStatus}</p>
    </section>
    <div class="workbench">
      <section class="program">
        <div class="panel-heading">
          <h2>Your program</h2>
          <button disabled={program.rules.length >= 8} onclick={() => addRule()}
            >＋ Add rule</button
          >
        </div>
        <p class="muted">
          Up to 8 rules. If several match, the later rule wins for each output.
          Fast repeats have a short cooldown.
        </p>
        {#each program.rules as rule, index}<article
            class="rule"
            aria-label={`Rule ${index + 1}`}
          >
            <div class="panel-heading">
              <strong>Rule {index + 1}</strong><button
                disabled={program.rules.length === 1}
                onclick={() => {
                  void stop();
                  program = {
                    rules: program.rules.filter((_, i) => i !== index),
                  };
                  dirty();
                }}>Remove rule {index + 1}</button
              >
            </div>
            <label class="input-block"
              >WHEN<select
                aria-label={`Rule ${index + 1} input`}
                value={rule.trigger}
                onchange={(event) =>
                  change(index, {
                    trigger: event.currentTarget.value as Rule["trigger"],
                  })}
                ><option value="A">button A is pressed</option><option value="B"
                  >button B is pressed</option
                ><option value="near">Distance crosses closer than…</option
                ><option value="far"
                  >Distance crosses to the boundary or farther…</option
                ></select
              ></label
            >
            {#if rule.trigger === "near" || rule.trigger === "far"}<label
                >Boundary (cm)<input
                  aria-label={`Rule ${index + 1} boundary (cm)`}
                  type="number"
                  min="2"
                  max="400"
                  value={rule.thresholdMm / 10}
                  onchange={(event) =>
                    change(index, {
                      thresholdMm: Number(event.currentTarget.value) * 10,
                    })}
                /></label
              >
              <p class="muted">
                Move across the boundary after Run; staying still does not
                repeat the action.
              </p>{/if}
            <label class="check"
              ><input
                aria-label={`Rule ${index + 1} condition enabled`}
                type="checkbox"
                checked={rule.conditionMm !== null}
                onchange={(event) =>
                  change(index, {
                    conditionMm: event.currentTarget.checked ? 200 : null,
                  })}
              />ONLY IF Distance is closer than…</label
            >
            {#if rule.conditionMm !== null}<label
                >Condition distance (cm)<input
                  aria-label={`Rule ${index + 1} condition (cm)`}
                  type="number"
                  min="2"
                  max="400"
                  value={rule.conditionMm / 10}
                  onchange={(event) =>
                    change(index, {
                      conditionMm: Number(event.currentTarget.value) * 10,
                    })}
                /></label
              >{/if}
            <div class="output-block">
              <label class="check"
                ><input
                  aria-label={`Rule ${index + 1} light enabled`}
                  type="checkbox"
                  checked={rule.color !== null}
                  onchange={(event) =>
                    change(index, {
                      color: event.currentTarget.checked ? "#4986ff" : null,
                    })}
                />THEN make Pixels a color briefly</label
              >{#if rule.color}<label
                  >Light color<input
                    aria-label={`Rule ${index + 1} color`}
                    type="color"
                    value={rule.color}
                    oninput={(event) =>
                      change(index, { color: event.currentTarget.value })}
                  /></label
                >{/if}<label class="check"
                ><input
                  aria-label={`Rule ${index + 1} chirp`}
                  type="checkbox"
                  checked={rule.chirp}
                  onchange={(event) =>
                    change(index, { chirp: event.currentTarget.checked })}
                />Play a short chirp</label
              >
            </div>
          </article>{/each}
        <button onclick={() => stop(true)}>Stop program</button>
        <p>
          Editing or changing screens stops execution. C is reserved for Stop. A
          program runs on this Mac, not autonomously on the Board.
        </p>
        <details open={live.trace.length > 0}>
          <summary>What happened?</summary>{#each live.trace as line}<p>
              {line}
            </p>{/each}
        </details>
        <details class="palette">
          <summary>How these blocks work</summary>
          <p>
            Use Add rule, then choose its WHEN input. Add an ONLY IF distance
            condition if needed, and choose light, chirp or both in THEN. No
            code or dragging required.
          </p>
        </details>
        <section class="save-panel">
          <div class="row">
            <button
              disabled={!libraryReady ||
                saving ||
                live.state === "stopping" ||
                !title.trim()}
              onclick={() => save(true)}>Save a separate copy</button
            ><button onclick={fresh}>New creation</button>
          </div>
        </section>
      </section>
      <aside>
        {@render preview()}
        <section class="quick-inputs">
          <h2>Try your inputs</h2>
          <p class="health">Buttons: {health("buttons")}</p>
          {@render buttonInput()}
          <h3>Distance</h3>
          <p class="health">{health("distance")}</p>
          {@render distanceInput()}
        </section>
      </aside>
    </div>
  {:else if screen === "missions"}
    <section class="mission">
      <span class="eyebrow">AN IDEA TO MAKE YOUR OWN</span>
      <h1 tabindex="-1">Make a treasure guardian</h1>
      <p class="lead">
        Build something that notices a visitor. When they come close, its light
        changes.
      </p>
      <section class="learn">
        <h2>You’ll learn</h2>
        <p>
          Sensor → distance rule → light. Predict a response, test it, then
          change the rule.
        </p>
      </section>
      <p>
        <strong>Start with:</strong> Distance + Pixels. Add Buttons and Buzzer when
        you want more control and sound.
      </p>
      <p>
        Any materials, one maker or a whole Crew. Leave an opening for the
        sensor; disconnect power before changing wiring. UNO Q maker I/O uses
        3.3 V logic.
      </p>
      <button class="primary" onclick={guardian}>Make my guardian →</button>
      <p>
        No required names, roles or written answers. Your starter opens in the
        same block workspace as free creation.
      </p>
      <details>
        <summary>Earlier guided adventure and saved progress</summary>
        <p>
          The original ten-stage Mission and its saves have not been changed or
          marked complete by this starter.
        </p>
        <a
          href="/legacy"
          onclick={(event) => {
            event.preventDefault();
            void requestReplacement({ kind: "legacy" });
          }}>Open earlier Mission progress →</a
        >
      </details>
    </section>
  {/if}
  {#if screen === "library"}<section class="saved">
      <h1 tabindex="-1">My creations</h1>
      <p>
        Current Creation: <strong>{title}</strong>{unsaved
          ? " · unsaved changes"
          : ""}
      </p>
      <button onclick={() => navigate(createView)}>Return to my Creation</button
      >
      <div class="panel-heading">
        <h2>Your saved creations · {library.creations.length}/24</h2>
        <button onclick={fresh}>Start a new creation</button>
      </div>
      {#if library.creations.length}<div class="row">
          {#each library.creations as entry}<article class="saved-card">
              <button onclick={() => openCreation(entry.id)}
                >{entry.title}</button
              >
              <p class="muted">
                {entry.program.rules.length}
                {entry.program.rules.length === 1 ? "rule" : "rules"} · Opens stopped
              </p>
            </article>{/each}
        </div>{:else}<p>
          {libraryReady
            ? "No creations saved yet."
            : "Saved library is not available yet. Existing work has not been replaced."}
        </p>{/if}
      <p class="muted">
        Opening or copying a Creation never runs it. Earlier Mission saves are
        separate.
      </p>
    </section>{/if}
  <footer>
    <p>{message}</p>
    <span
      >Disconnect power before changing wiring. UNO Q maker I/O uses 3.3 V
      logic; never apply 5 V to analog inputs. No arbitrary port/power controls;
      more hardware needs explicit support.</span
    >
  </footer>
</main>

<style>
  :global(body) {
    background: var(--canvas);
  }
  header {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 16px 26px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .brand {
    border: 0;
    padding: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    text-decoration: none;
    font-size: 24px;
    font-weight: 850;
    color: var(--primary);
  }
  nav,
  .row,
  .board-dock,
  .panel-heading {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .creation-views {
    margin-bottom: 20px;
  }
  h1:focus {
    outline: none;
  }
  .board-dock {
    margin-left: auto;
  }
  .board-dock span {
    font-size: 12px;
    color: var(--muted);
  }
  button {
    min-height: 44px;
    padding: 10px 15px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--ink);
    font: inherit;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    border-color: var(--primary);
    background: var(--primary-soft);
  }
  button:disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }
  .active {
    color: var(--primary);
    background: var(--primary-soft);
  }
  .primary {
    background: var(--primary);
    color: white;
    border-color: var(--primary);
    font-weight: 750;
  }
  .primary:hover:not(:disabled) {
    background: var(--primary-hover);
    color: white;
  }
  .editor-toolbar {
    position: sticky;
    top: var(--header-height);
    z-index: 4;
    display: grid;
    grid-template-columns: minmax(180px, 1fr) auto;
    align-items: center;
    gap: 4px 24px;
    padding: 12px 20px;
    margin-bottom: 24px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .editor-toolbar label {
    margin: 0;
  }
  .editor-toolbar p {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .draft-decision {
    padding: 20px;
    margin-bottom: 24px;
    border: 2px solid var(--primary);
    border-radius: 16px;
    background: var(--surface);
    scroll-margin-top: 160px;
  }
  #keep-editing {
    scroll-margin-top: 180px;
  }
  .stop {
    background: var(--danger-bg);
    color: var(--danger);
    border-color: #d99492;
    font-weight: 800;
  }
  main {
    max-width: 1500px;
    margin: auto;
    padding: 30px;
  }
  h1 {
    font-size: clamp(28px, 3.3vw, 42px);
    line-height: 1.2;
    letter-spacing: -1px;
    margin: 12px 0;
  }
  h2 {
    font-size: 21px;
    margin: 4px 0 16px;
  }
  h3 {
    font-size: 17px;
    margin: 4px 0;
  }
  p {
    line-height: 1.6;
  }
  small,
  .muted {
    font-size: 12px;
    color: var(--muted);
  }
  .eyebrow {
    font-size: 11px;
    font-weight: 850;
    letter-spacing: 0.08em;
    color: var(--primary);
  }
  .heading {
    margin: 10px 0 28px;
  }
  .heading p {
    color: var(--muted);
  }
  .playground {
    display: grid;
    grid-template-columns: 1.4fr 0.6fr;
    gap: 24px;
  }
  .module-panel,
  .preview,
  .program,
  .next-step,
  .saved,
  .mission {
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--surface);
  }
  .panel-heading {
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .module-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
  }
  .module-grid article {
    padding: 14px 0;
    border-bottom: 1px solid var(--border);
  }
  .module-grid p {
    font-size: 13px;
  }
  .health {
    color: var(--teal);
    font-size: 12px;
  }
  .reading {
    font-size: 24px !important;
    font-weight: 800;
  }
  .pressed {
    background: var(--primary-soft);
  }
  .module-grid span {
    font-size: 12px;
  }
  label {
    display: block;
    margin: 14px 0;
    font-size: 14px;
  }
  input,
  select {
    font: inherit;
    color: var(--ink);
  }
  select,
  input[type="number"],
  .editor-toolbar input {
    display: block;
    width: 100%;
    min-height: 44px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    padding: 8px;
    margin-top: 8px;
  }
  input[type="range"] {
    width: 100%;
    display: block;
    margin-top: 12px;
    accent-color: var(--primary);
  }
  input[type="color"] {
    display: block;
    width: 70px;
    height: 44px;
    margin-top: 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 3px;
  }
  .check {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .check input {
    width: 18px;
    height: 18px;
    accent-color: var(--primary);
  }
  .preview {
    text-align: center;
    align-self: start;
  }
  .invention {
    width: 160px;
    height: 170px;
    border: 3px solid #d6cbea;
    background: var(--primary-soft);
    border-radius: 42px 42px 20px 20px;
    margin: 10px auto 24px;
    display: grid;
    place-items: center;
    padding: 15px;
  }
  .invention > span {
    font-size: 34px;
    letter-spacing: 15px;
    padding-left: 15px;
    color: var(--primary);
  }
  .lamp {
    width: 75px;
    height: 50px;
    border-radius: 14px;
  }
  .preview p {
    font-size: 13px;
  }
  .preview small {
    display: block;
    max-width: 280px;
    margin: auto;
  }
  .next-step {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 25px;
    margin-top: 24px;
  }
  .next-step p {
    margin-bottom: 0;
  }
  .workbench {
    display: grid;
    grid-template-columns: minmax(360px, 1fr) 330px;
    gap: 24px;
    align-items: start;
  }
  .palette {
    margin-top: 16px;
    padding: 12px 18px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
  }
  .palette p {
    font-size: 13px;
  }
  .workbench aside {
    position: sticky;
    top: 100px;
  }
  .workbench .preview {
    padding: 14px;
  }
  .workbench .invention {
    width: 76px;
    height: 82px;
    border-radius: 24px 24px 12px 12px;
    margin: 0 auto 12px;
    padding: 6px;
  }
  .workbench .invention > span {
    font-size: 18px;
    letter-spacing: 4px;
    padding-left: 4px;
    white-space: nowrap;
    line-height: 1;
  }
  .workbench .lamp {
    width: 38px;
    height: 22px;
    border-radius: 7px;
  }
  .workbench .preview p {
    margin: 6px 0;
    font-size: 12px;
  }
  .workbench .preview small {
    font-size: 11px;
  }
  .quick-inputs {
    padding: 18px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 16px;
    margin-top: 14px;
  }
  .quick-inputs p {
    margin: 8px 0;
    font-size: 12px;
  }
  .quick-inputs label {
    margin: 10px 0;
  }
  .rule {
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    margin: 18px 0;
  }
  .input-block {
    background: var(--primary-soft);
    padding: 14px;
    border-radius: 9px;
  }
  .output-block {
    background: var(--teal-soft);
    padding: 4px 14px;
    border-radius: 9px;
  }
  .save-panel {
    border-top: 1px solid var(--border);
    margin-top: 24px;
    padding-top: 10px;
  }
  details {
    margin-top: 18px;
  }
  summary {
    cursor: pointer;
    color: var(--primary);
    font-weight: 750;
    padding: 8px 0;
  }
  .saved {
    margin-top: 28px;
  }
  .mission {
    max-width: 820px;
    margin: 20px auto;
  }
  .lead {
    font-size: 20px;
  }
  .learn {
    background: var(--primary-soft);
    border-left: 3px solid var(--primary);
    padding: 20px;
    margin: 24px 0;
  }
  .learn p {
    margin-bottom: 0;
  }
  .error {
    padding: 15px;
    border-radius: 10px;
    background: var(--danger-bg);
    color: var(--danger);
  }
  .connection-sheet {
    position: fixed;
    top: calc(var(--header-height) + 12px);
    right: 24px;
    width: min(580px, calc(100vw - 48px));
    max-height: calc(100vh - var(--header-height) - 24px);
    overflow: auto;
    z-index: 6;
    padding: 20px;
    border: 2px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: 0 12px 40px #26304b30;
  }
  .connection-sheet :global(.connections) {
    margin-top: 0;
  }
  .connection-sheet :global(h2) {
    font-size: 26px;
  }
  .parent-tasks {
    margin: 24px 0;
  }
  .saved-card {
    min-width: 220px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
  }
  footer {
    margin-top: 32px;
    color: var(--muted);
    font-size: 13px;
  }
  footer span {
    display: block;
    margin-top: 15px;
  }
  @media (max-width: 1100px) {
    header {
      flex-wrap: wrap;
      gap: 12px;
    }
    .board-dock {
      margin-left: 0;
      width: 100%;
    }
    .workbench {
      grid-template-columns: 1fr 300px;
    }
    .workbench aside {
      top: 150px;
    }
  }
  @media (max-width: 850px) {
    .workbench aside {
      position: static;
    }
    main {
      padding: 18px;
    }
    .playground,
    .workbench,
    .module-grid {
      grid-template-columns: 1fr;
    }
    .next-step {
      display: block;
    }
    .next-step button {
      margin-top: 18px;
    }
  }
</style>
