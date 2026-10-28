<script lang="ts">
  import ConnectionPanel from "$lib/board/ConnectionPanel.svelte";
  import type { ConnectionState } from "$lib/board/connection";
  import MissionPanel from "$lib/missions/MissionPanel.svelte";
  import { world, treasureGuardian } from "$lib/missions/catalog";
  import { SimulatorBoardClient } from "$lib/board/simulator";
  import type {
    BoardClient,
    BoardEvent,
    BoardSnapshot,
  } from "$lib/board/types";

  type Screen =
    "Home Base" | "World Map" | "Mission" | "Hardware Lab" | "Board Setup";
  const screens: { name: Screen; icon: string }[] = [
    { name: "Board Setup", icon: "☰" },
    { name: "Home Base", icon: "⌂" },
    { name: "World Map", icon: "✦" },
    { name: "Mission", icon: "◆" },
    { name: "Hardware Lab", icon: "⚙" },
  ];
  const simulator = new SimulatorBoardClient();
  let board = $state.raw<BoardClient>(simulator);
  let connectionState = $state<ConnectionState>({ phase: "simulator" });
  let boardError = $state("");
  let active = $state<Screen>("Home Base");
  let freeBuild = $state(false);
  let snapshot = $state<BoardSnapshot | null>(null);
  let lastEvent = $state<BoardEvent | null>(null);

  async function refresh(selected = board) {
    try {
      const next = await selected.getSnapshot();
      if (selected === board) {
        snapshot = next;
        boardError = "";
      }
    } catch {
      boardError =
        "Board unavailable. Check power and connections; Board Setup offers USB/manual recovery.";
    }
  }
  $effect(() => {
    const selected = board;
    let polling = false;
    void refresh(selected);
    const unsubscribe = selected.subscribe((event) => {
      lastEvent = event;
    });
    const timer = setInterval(() => {
      if (polling || active !== "Hardware Lab") return;
      polling = true;
      void refresh(selected).finally(() => {
        polling = false;
      });
    }, 250);
    return () => {
      clearInterval(timer);
      unsubscribe();
    };
  });
  async function glow(color: string) {
    try {
      await board.renewControl();
      await board.command({
        operation: "pixels.set",
        arguments: { color, brightness: 0.35 },
      });
      await refresh();
    } catch {
      boardError =
        "The light command did not finish. Check Board health and reconnect in Board Setup.";
    }
  }
</script>

<svelte:head>
  <title>Tinkeroo — Build. Code. Imagine. Together.</title>
  <meta
    name="description"
    content="A playful, local-first maker adventure for one family Crew."
  />
</svelte:head>

<div
  style="padding: 12px 24px; background: var(--primary-soft); font-size: 14px;"
>
  Earlier guided adventure — your original saves are preserved.
  <a href="/">Return to my Creation →</a>
</div>
<div class="app-shell">
  <aside>
    <div class="brand">
      <img src="/icon.svg" alt="Tinkeroo" width="44" height="44" />
      <div>
        <strong>Tinkeroo</strong><small>Little ideas. Big possibilities.</small>
      </div>
    </div>
    <nav aria-label="Main navigation">
      {#each screens as screen}
        <button
          class:active={active === screen.name}
          aria-current={active === screen.name ? "page" : undefined}
          onclick={() => (active = screen.name)}
        >
          <span>{screen.icon}</span>{screen.name}
        </button>
      {/each}
    </nav>
    <div class="sidebar-note">
      <span aria-hidden="true">✧</span> One Crew.<br />Lots of ways to create.
    </div>
    <div class="board-status">
      <span
        class="status-dot"
        class:attention={connectionState.phase === "failed" ||
          connectionState.phase === "searching"}
      ></span>
      <div>
        <strong
          >{connectionState.phase === "simulator"
            ? "Simulator ready"
            : connectionState.phase === "connected"
              ? "Board connected"
              : connectionState.phase === "searching"
                ? "Reconnecting Board…"
                : "Board needs attention"}</strong
        >
        <small
          >{connectionState.phase === "simulator"
            ? "No Board required"
            : "Board Setup for recovery"}</small
        >
      </div>
    </div>
  </aside>
  <main>
    <header>
      <div>
        <span class="eyebrow">YOUR MAKER ADVENTURE</span>
        <h1>{active}</h1>
      </div>
      <div class="crew">
        <span>🧭</span><span>🧰</span><span>💻</span><b>Your Crew</b>
      </div>
    </header>
    {#if active === "Home Base"}
      <section class="hero">
        <div>
          <span class="eyebrow">BUILD. CODE. IMAGINE. TOGETHER.</span>
          <h2>Small ideas.<br />Wonderful inventions.</h2>
          <p>Your Crew brings the story. Let’s bring your guardian to life.</p>
          <button
            class="primary"
            data-testid="start-mission"
            onclick={() => (active = "Mission")}
            >Open Treasure Guardian →</button
          >
          <p class="gentle-note">
            Start or resume your saved Mission. Real Board outputs stay off
            until you explicitly enable Mission controls.
          </p>
          <button onclick={() => (active = "Board Setup")}
            >Connect or set up your Board</button
          >
        </div>
        <div class="maker-mark" aria-hidden="true">
          <img src="/icon.svg" alt="" width="152" height="152" /><span
            class="spark">✦</span
          ><span class="block"></span>
        </div>
      </section>
      <section class="cards" aria-label="Adventure overview">
        <article>
          <span class="card-icon amber">◆</span>
          <div>
            <small>YOUR MISSION</small>
            <h3>Treasure Guardian</h3>
            <p>Build a hideaway. Give it lights, sounds, and a personality.</p>
          </div>
        </article>
        <article>
          <span class="card-icon blue">⚙</span>
          <div>
            <small>READY TO EXPERIMENT</small>
            <h3>{snapshot?.capabilities.length ?? 0} capabilities</h3>
            <p>Practice with the simulator or your selected Board.</p>
          </div>
        </article>
        <article>
          <span class="card-icon green">★</span>
          <div>
            <small>EVERY MAKER MATTERS</small>
            <h3>One shared adventure</h3>
            <p>
              Explorer, Inventor, and Hacker: different challenges, together.
            </p>
          </div>
        </article>
      </section>
    {:else if active === "World Map"}
      <section class="panel">
        <span class="eyebrow">ADVENTURE PATH</span>
        <h2>Follow your curiosity.</h2>
        <p>
          Treasure Guardian is the playable Mission today. Other adventures are
          marked coming later. Afterward, try a Free Build invitation.
        </p>
        <div class="mission-path">
          {#each world.stops as stop, index}
            <button
              class:current={stop.missionId === treasureGuardian.id}
              disabled={stop.missionId === null}
              onclick={() => {
                if (stop.missionId === "free-build") freeBuild = !freeBuild;
                else active = "Mission";
              }}
            >
              <b>{index + 1}</b><span
                >{stop.title}{stop.missionId === null
                  ? " — coming later"
                  : ""}</span
              >
            </button>
          {/each}
        </div>
        {#if freeBuild}
          <h3>Free Build invitations</h3>
          {#each treasureGuardian.freeBuild as invitation}<p>
              <strong>{invitation.prompt}</strong>: {invitation.components.join(
                ", ",
              )} — {invitation.constraint}.
            </p>{/each}
        {/if}
      </section>
    {:else if active === "Mission"}
      {#key board}<MissionPanel
          {board}
          simulation={board === simulator ? simulator.controls : undefined}
          onSetup={() => (active = "Board Setup")}
          onHome={() => (active = "Home Base")}
        />{/key}
    {:else if active === "Hardware Lab"}
      <section class="panel">
        <span class="eyebrow">TRY. NOTICE. TRY AGAIN.</span>
        <h2>
          {board === simulator ? "Simulator" : "Connected Board"} Hardware Lab
        </h2>
        {#if boardError}<p role="alert">{boardError}</p>{/if}
        <p>
          These controls feed the same <code>BoardClient</code> event interface used
          by connected hardware.
        </p>
        <div class="lab-grid">
          {#if board === simulator}
            <label
              >Distance <strong
                >{snapshot?.observables.distanceMillimeters ?? "—"} mm</strong
              ><input
                type="range"
                min="20"
                max="1000"
                value={snapshot?.observables.distanceMillimeters ?? 600}
                oninput={(event) =>
                  simulator.controls.setDistance(
                    Number(event.currentTarget.value),
                  )}
              /></label
            >
            <div>
              <strong>Buttons</strong>
              <div class="button-row">
                {#each [0, 1, 2] as button}<button
                    onclick={() => simulator.controls.pressButton(button)}
                    >{String.fromCharCode(65 + button)}</button
                  >{/each}
              </div>
            </div>
          {:else}<p>
              Distance: {snapshot?.observables.distanceMillimeters ?? "—"} mm. Use
              the physical Board buttons.
            </p>{/if}
          <div>
            <strong>Pixels</strong>
            <div class="button-row">
              <button class="green-button" onclick={() => glow("#22c55e")}
                >Safe</button
              ><button class="amber-button" onclick={() => glow("#ffb703")}
                >Warn</button
              ><button class="red-button" onclick={() => glow("#ef4444")}
                >Alert</button
              >
            </div>
          </div>
        </div>
        {#if lastEvent}<pre>{JSON.stringify(lastEvent, null, 2)}</pre>{/if}
      </section>
    {/if}
    <ConnectionPanel
      visible={active === "Board Setup"}
      onMission={() => (active = "Mission")}
      onConnected={(client) => {
        board = client;
      }}
      onSimulator={() => {
        board = simulator;
      }}
      onState={(state) => {
        connectionState = state;
      }}
    />
  </main>
</div>

<style>
  .app-shell {
    display: grid;
    grid-template-columns: 238px minmax(0, 1fr);
    min-height: 100vh;
  }
  aside {
    position: sticky;
    top: 0;
    height: 100vh;
    display: flex;
    flex-direction: column;
    padding: 28px 16px;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  .brand {
    display: flex;
    gap: 11px;
    align-items: center;
    padding: 0 6px 34px;
  }
  .brand img {
    border-radius: 14px;
  }
  .brand strong {
    font-size: 24px;
    letter-spacing: -0.8px;
    color: var(--primary);
  }
  .brand small {
    font-size: 10px;
    color: var(--muted);
  }
  .brand strong,
  .brand small,
  .board-status strong,
  .board-status small {
    display: block;
  }
  nav {
    display: grid;
    gap: 8px;
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 13px;
    border: 1px solid transparent;
    border-radius: 12px;
    color: var(--muted);
    background: transparent;
    text-align: left;
  }
  nav button:hover {
    background: var(--canvas);
    color: var(--primary);
  }
  nav button.active {
    background: var(--primary-soft);
    color: var(--primary);
    border-color: #d2c7f5;
    box-shadow: inset 3px 0 var(--primary);
  }
  nav span {
    width: 22px;
    font-size: 20px;
    text-align: center;
  }
  .sidebar-note {
    margin: 38px 14px;
    color: var(--muted);
    font-size: 13px;
  }
  .sidebar-note span {
    float: left;
    font-size: 32px;
    margin-right: 10px;
    color: var(--teal);
  }
  .board-status {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-top: auto;
    padding: 13px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--canvas);
  }
  .status-dot {
    width: 9px;
    height: 9px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--teal);
  }
  .status-dot.attention {
    background: var(--warning-ink);
  }
  .board-status strong {
    font-size: 12px;
  }
  .board-status small {
    color: var(--muted);
    font-size: 11px;
  }
  main {
    padding: 0 36px 48px;
    min-width: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    min-height: 88px;
    border-bottom: 1px solid var(--border);
  }
  h1,
  h2,
  h3,
  p {
    margin-top: 0;
  }
  h1 {
    margin-bottom: 0;
    font-size: 30px;
    letter-spacing: -0.7px;
  }
  h2 {
    margin: 12px 0 18px;
    font-size: clamp(34px, 4vw, 50px);
    line-height: 1.15;
    letter-spacing: -1.3px;
  }
  .eyebrow,
  article small {
    color: var(--primary);
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.09em;
  }
  .crew {
    display: flex;
    align-items: center;
    color: var(--muted);
    font-size: 13px;
  }
  .crew span {
    display: grid;
    width: 32px;
    height: 32px;
    margin-left: -4px;
    place-items: center;
    border: 2px solid var(--canvas);
    border-radius: 50%;
    background: var(--primary-soft);
  }
  .crew b {
    margin-left: 9px;
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 170px;
    gap: 20px;
    align-items: center;
    margin: 32px 0 24px;
    padding: 38px;
    border: 1px solid #e0d9f5;
    border-radius: 24px;
    background: var(--surface-soft);
  }
  .hero p {
    color: var(--muted);
    max-width: 460px;
    font-size: 17px;
  }
  .hero .gentle-note {
    margin: 14px 0 0;
    font-size: 13px;
  }
  .primary {
    padding: 14px 20px;
    border: 0;
    border-radius: 12px;
    color: white;
    background: var(--primary);
  }
  .primary:hover {
    background: var(--primary-hover);
  }
  .maker-mark {
    position: relative;
    padding: 30px 0;
  }
  .maker-mark img {
    position: relative;
    z-index: 1;
    border-radius: 38px;
    transform: rotate(-6deg);
  }
  .spark {
    position: absolute;
    right: -5px;
    top: 0;
    color: var(--sunshine-ink);
    font-size: 45px;
  }
  .block {
    position: absolute;
    width: 90px;
    height: 75px;
    right: -2px;
    bottom: 12px;
    border-radius: 22px;
    background: var(--sunshine);
    transform: rotate(12deg);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }
  .cards article {
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .cards h3 {
    margin: 14px 0 8px;
    font-size: 18px;
  }
  .cards p {
    margin: 0;
    color: var(--muted);
    font-size: 14px;
  }
  .cards small {
    display: block;
    margin-top: 16px;
    font-size: 10px;
  }
  .card-icon {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    border-radius: 13px;
    font-size: 21px;
  }
  .amber {
    background: #fff3cc;
    color: var(--sunshine-ink);
  }
  .blue {
    background: var(--primary-soft);
    color: var(--primary);
  }
  .green {
    background: var(--teal-soft);
    color: var(--teal);
  }
  .panel {
    margin-top: 32px;
    padding: 32px;
    border: 1px solid var(--border);
    border-radius: 20px;
    background: var(--surface);
  }
  .panel > p {
    max-width: 740px;
    color: var(--muted);
  }
  .panel h2 {
    font-size: 36px;
  }
  .mission-path {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    margin: 28px 0;
  }
  .mission-path button {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--canvas);
    color: var(--muted);
    text-align: left;
  }
  .mission-path button:disabled {
    opacity: 0.7;
  }
  .mission-path b {
    display: grid;
    min-width: 34px;
    height: 34px;
    place-items: center;
    border-radius: 50%;
    background: var(--border);
    color: var(--ink);
  }
  .mission-path .current {
    background: var(--primary-soft);
    border: 2px solid var(--primary);
    color: var(--primary);
  }
  .mission-path .current b {
    background: var(--primary);
    color: white;
  }
  .lab-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
    margin-top: 26px;
  }
  .lab-grid > * {
    padding: 18px;
    border-radius: 14px;
    background: var(--canvas);
  }
  .lab-grid label strong,
  .lab-grid > div > strong {
    display: block;
    margin-bottom: 14px;
  }
  .lab-grid input {
    width: 100%;
  }
  .button-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .button-row button {
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface);
  }
  .button-row .green-button {
    color: #14663e;
    background: #e4f5e9;
  }
  .button-row .amber-button {
    color: var(--warning-ink);
    background: var(--warning-bg);
  }
  .button-row .red-button {
    color: var(--danger);
    background: var(--danger-bg);
  }
  pre {
    overflow: auto;
    padding: 18px;
    border-radius: 12px;
    color: var(--code-ink);
    background: var(--code-bg);
  }
  code {
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--surface-soft);
  }
  @media (max-width: 1050px) {
    main {
      padding-inline: 24px;
    }
    .hero {
      grid-template-columns: 1fr;
      padding: 30px;
    }
    .maker-mark {
      display: none;
    }
    .cards {
      grid-template-columns: 1fr;
    }
    .lab-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
