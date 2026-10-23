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
    "Home Base" | "World Map" | "Mission" | "Hardware Lab" | "Parent Settings";

  const screens: { name: Screen; icon: string }[] = [
    { name: "Home Base", icon: "⌂" },
    { name: "World Map", icon: "✦" },
    { name: "Mission", icon: "◆" },
    { name: "Hardware Lab", icon: "⚙" },
    { name: "Parent Settings", icon: "☰" },
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
        "Board unavailable. Check power and connections; Parent Settings offers USB/manual recovery.";
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
        "The light command did not finish. Check Board health and reconnect in Parent Settings.";
    }
  }
</script>

<svelte:head>
  <title>TinkerCrew — Build. Code. Imagine.</title>
  <meta
    name="description"
    content="A local-first cooperative maker game for one family crew."
  />
</svelte:head>

<div class="app-shell">
  <aside>
    <div class="brand">
      <span class="brand-mark">TC</span>
      <div><strong>TinkerCrew</strong><small>Build. Code. Imagine.</small></div>
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
    <div class="board-status">
      <span
        class="status-dot"
        style:background={connectionState.phase === "failed"
          ? "#ffb703"
          : "#4ade80"}
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
            : "Parent Settings for recovery"}</small
        >
      </div>
    </div>
  </aside>

  <main>
    <header>
      <div>
        <span class="eyebrow">FAMILY CREW</span>
        <h1>{active}</h1>
      </div>
      <div class="crew">
        <span>🧭</span><span>🧰</span><span>💻</span><b>Dragon Crew</b>
      </div>
    </header>

    {#if active === "Home Base"}
      <section class="hero">
        <div>
          <span class="eyebrow">WELCOME BACK, MAKERS</span>
          <h2>Your next invention is waiting.</h2>
          <p>Build together, make it move, then remix the rules.</p>
          <button class="primary" onclick={() => (active = "World Map")}
            >Explore the world map →</button
          >
        </div>
        <div class="orb" aria-hidden="true"><span>✦</span></div>
      </section>
      <section class="cards">
        <article>
          <span class="card-icon amber">◆</span>
          <div>
            <small>NEXT MISSION</small>
            <h3>Treasure Guardian</h3>
            <p>Wake the cave guardian with distance, lights, and sound.</p>
          </div>
        </article>
        <article>
          <span class="card-icon blue">⚙</span>
          <div>
            <small>HARDWARE</small>
            <h3>{snapshot?.capabilities.length ?? 0} capabilities</h3>
            <p>Practice safely with the built-in simulator.</p>
          </div>
        </article>
        <article>
          <span class="card-icon green">★</span>
          <div>
            <small>CREW PROGRESS</small>
            <h3>One shared adventure</h3>
            <p>Explorer, Inventor, and Hacker tracks stay together.</p>
          </div>
        </article>
      </section>
    {:else if active === "World Map"}
      <section class="panel">
        <span class="eyebrow">ADVENTURE PATH</span>
        <h2>From first signal to free invention</h2>
        <div class="mission-path">
          {#each world.stops as stop, index}<button
              class:current={stop.missionId === treasureGuardian.id}
              disabled={stop.missionId === null}
              onclick={() => {
                if (stop.missionId === "free-build") freeBuild = !freeBuild;
                else active = "Mission";
              }}
              ><b>{index + 1}</b><span
                >{stop.title}{stop.missionId === null
                  ? " — coming later"
                  : ""}</span
              ></button
            >{/each}
        </div>
        {#if freeBuild}
          <h3>Free Build invitations</h3>
          {#each treasureGuardian.freeBuild as invitation}
            <p>
              <strong>{invitation.prompt}</strong>: {invitation.components.join(
                ", ",
              )} — {invitation.constraint}.
            </p>
          {/each}
        {/if}
      </section>
    {:else if active === "Mission"}
      {#key board}<MissionPanel
          {board}
          simulation={board === simulator ? simulator.controls : undefined}
        />{/key}
    {:else if active === "Hardware Lab"}
      <section class="panel">
        <span class="eyebrow">BOARD ADAPTER</span>
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
      visible={active === "Parent Settings"}
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
  :global(*) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    min-width: 900px;
    background: #f5f1e8;
    color: #18302b;
    font-family: Inter, ui-rounded, "Avenir Next", system-ui, sans-serif;
  }
  button {
    font: inherit;
  }
  .app-shell {
    display: grid;
    grid-template-columns: 245px 1fr;
    min-height: 100vh;
  }
  aside {
    display: flex;
    flex-direction: column;
    padding: 28px 18px;
    color: #f9f6ed;
    background: #173b35;
  }
  .brand {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 0 10px 30px;
  }
  .brand-mark {
    display: grid;
    width: 40px;
    height: 40px;
    place-items: center;
    border: 2px solid #ffb703;
    border-radius: 14px 8px;
    color: #ffb703;
    font-weight: 900;
  }
  .brand strong,
  .brand small,
  .board-status strong,
  .board-status small {
    display: block;
  }
  .brand strong {
    font-size: 18px;
  }
  .brand small {
    margin-top: 2px;
    color: #a8c3bc;
    font-size: 10px;
    letter-spacing: 0.08em;
  }
  nav {
    display: grid;
    gap: 7px;
  }
  nav button {
    display: flex;
    gap: 12px;
    align-items: center;
    width: 100%;
    padding: 12px 14px;
    border: 0;
    border-radius: 10px;
    color: #bcd0cb;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  nav button:hover,
  nav button.active {
    color: white;
    background: #285148;
  }
  nav button.active {
    box-shadow: inset 3px 0 #ffb703;
  }
  nav span {
    width: 20px;
    color: #ffb703;
    text-align: center;
  }
  .board-status {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-top: auto;
    padding: 14px;
    border: 1px solid #3b635a;
    border-radius: 12px;
  }
  .status-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 12px #4ade80;
  }
  .board-status strong {
    font-size: 12px;
  }
  .board-status small {
    color: #9bb9b1;
    font-size: 10px;
  }
  main {
    padding: 0 48px 48px;
    overflow: hidden;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    min-height: 110px;
    border-bottom: 1px solid #d9d2c3;
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
  }
  h2 {
    max-width: 650px;
    margin-bottom: 14px;
    font-family: Georgia, serif;
    font-size: clamp(38px, 5vw, 64px);
    line-height: 1.02;
  }
  .eyebrow,
  article small {
    color: #b66926;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.15em;
  }
  .crew {
    display: flex;
    align-items: center;
    gap: 0;
    color: #405c55;
    font-size: 13px;
  }
  .crew span {
    display: grid;
    width: 34px;
    height: 34px;
    margin-left: -5px;
    place-items: center;
    border: 2px solid #f5f1e8;
    border-radius: 50%;
    background: #dfd7c6;
  }
  .crew b {
    margin-left: 9px;
  }
  .hero {
    position: relative;
    display: flex;
    min-height: 340px;
    margin: 38px 0 24px;
    padding: 52px;
    align-items: center;
    overflow: hidden;
    border-radius: 24px;
    color: white;
    background: linear-gradient(120deg, #24584d, #163a34);
  }
  .hero p {
    max-width: 480px;
    color: #c9ddd8;
    font-size: 18px;
  }
  .hero .eyebrow {
    color: #ffc449;
  }
  .primary {
    padding: 13px 20px;
    border: 0;
    border-radius: 10px;
    color: #263526;
    background: #ffbd39;
    font-weight: 800;
    cursor: pointer;
  }
  .orb {
    position: absolute;
    right: -40px;
    width: 330px;
    height: 330px;
    border: 1px solid #48756b;
    border-radius: 50%;
    box-shadow: inset 0 0 80px #2d685c;
  }
  .orb::before,
  .orb::after {
    position: absolute;
    content: "";
    border: 1px solid #48756b;
    border-radius: 50%;
    inset: 38px;
  }
  .orb::after {
    inset: 88px;
  }
  .orb span {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #ffbd39;
    font-size: 86px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 18px;
  }
  .cards article {
    display: flex;
    gap: 16px;
    min-height: 145px;
    padding: 22px;
    border: 1px solid #dfd8ca;
    border-radius: 16px;
    background: #fffdf7;
  }
  .cards h3 {
    margin: 5px 0 8px;
  }
  .cards p {
    color: #6c7a75;
    font-size: 13px;
    line-height: 1.5;
  }
  .card-icon {
    display: grid;
    flex: 0 0 43px;
    height: 43px;
    place-items: center;
    border-radius: 12px;
  }
  .amber {
    background: #fff0cd;
    color: #b66926;
  }
  .blue {
    background: #dceef4;
    color: #286b80;
  }
  .green {
    background: #dcf1e4;
    color: #267247;
  }
  .panel {
    margin-top: 38px;
    padding: 45px;
    border: 1px solid #ded6c7;
    border-radius: 22px;
    background: #fffdf7;
  }
  .panel > p {
    max-width: 740px;
    color: #60716b;
    font-size: 17px;
    line-height: 1.6;
  }
  .panel h2 {
    font-size: 46px;
  }
  .mission-path {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 15px;
    margin-top: 32px;
  }
  .mission-path button {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 20px;
    border: 1px solid #d9d2c3;
    border-radius: 14px;
    background: #f8f4eb;
    text-align: left;
  }
  .mission-path b {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border-radius: 50%;
    color: white;
    background: #789087;
  }
  .mission-path .current {
    border-color: #dc9c33;
    box-shadow: 0 5px 20px #d5aa6533;
    cursor: pointer;
  }
  .mission-path .current b {
    background: #d6842c;
  }
  .lab-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin-top: 26px;
  }
  .lab-grid > * {
    padding: 20px;
    border-radius: 14px;
    background: #f4f0e7;
  }
  .lab-grid label strong,
  .lab-grid > div > strong {
    display: block;
    margin-bottom: 14px;
  }
  .lab-grid input {
    width: 100%;
    accent-color: #d6842c;
  }
  .button-row {
    display: flex;
    gap: 8px;
  }
  .button-row button {
    padding: 9px 13px;
    border: 1px solid #cfc6b7;
    border-radius: 8px;
    background: white;
    cursor: pointer;
  }
  .green-button {
    color: #168044;
  }
  .amber-button {
    color: #b66926;
  }
  .red-button {
    color: #bb3030;
  }
  pre {
    overflow: auto;
    padding: 18px;
    border-radius: 12px;
    color: #d9efe9;
    background: #173b35;
  }
  code {
    padding: 2px 5px;
    border-radius: 4px;
    background: #e8e3d8;
  }
  @media (max-width: 1050px) {
    main {
      padding-inline: 28px;
    }
    .cards {
      grid-template-columns: 1fr;
    }
    .orb {
      opacity: 0.35;
    }
    .hero > div:first-child {
      z-index: 1;
    }
    .lab-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
