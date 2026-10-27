<script lang="ts">
  // Throwaway UI question: which structure makes connect → try → invent obvious?
  // Three layouts on this isolated route: the existing Home mounts native connection
  // effects, so embedding there would violate this prototype's no-credentials scope.
  import { dev } from "$app/environment";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { onDestroy } from "svelte";
  import PrototypeSwitcher from "$lib/prototypes/PrototypeSwitcher.svelte";

  type Trigger = "A" | "B" | "distance";
  type Rule = {
    trigger: Trigger;
    threshold: number;
    color: string;
    chirp: boolean;
  };
  const variants = ["A", "B", "C"];
  const names = [
    "Studio · rule cards",
    "Workbench · click-to-add blocks",
    "Playground · try first",
  ];
  const variant = $derived(
    variants.includes(page.url.searchParams.get("variant") ?? "")
      ? page.url.searchParams.get("variant")!
      : "A",
  );
  let connected = $state(false);
  let running = $state(false);
  let distance = $state(60);
  let rule = $state<Rule>({
    trigger: "A",
    threshold: 20,
    color: "#4986ff",
    chirp: false,
  });
  let modules = $state({
    buttons: true,
    distance: true,
    pixels: true,
    buzzer: true,
  });
  let pixel = $state<string | null>(null);
  let chirping = $state(false);
  let lastButton = $state("");
  let message = $state(
    "Start by connecting the demo Board. Nothing here controls real hardware.",
  );
  let trace = $state<string[]>([]);
  let mission = $state(false);
  let missionOpen = $state(false);
  let showState = $state(false);
  let saved = $state<{ name: string; rule: Rule; mission: boolean }[]>([]);
  let lightTimer: ReturnType<typeof setTimeout> | undefined;
  let chirpTimer: ReturnType<typeof setTimeout> | undefined;
  let buttonTimer: ReturnType<typeof setTimeout> | undefined;
  const allowed = $derived(
    connected &&
      modules.pixels &&
      (rule.trigger === "distance" ? modules.distance : modules.buttons) &&
      (!rule.chirp || modules.buzzer),
  );
  const prototypeSnapshot = $derived({
    layout: variant,
    simulated: true,
    connected,
    running,
    modules,
    input: { distanceCm: distance, lastButton },
    rule,
    outputs: { pixel, chirping },
    mission,
    demoSaves: saved.length,
  });

  function note(text: string) {
    message = text;
    trace = [text, ...trace].slice(0, 5);
  }
  function stop(announce = true) {
    running = false;
    pixel = null;
    chirping = false;
    clearTimeout(lightTimer);
    clearTimeout(chirpTimer);
    if (announce) note("Stopped. All simulated outputs are off.");
  }
  function connect() {
    stop(false);
    connected = !connected;
    note(
      connected
        ? "Demo Board ready. Try a module, then make a rule."
        : "Demo Board disconnected. Your rule is still here; outputs are off.",
    );
  }
  function light(color: string) {
    if (!connected || !modules.pixels) return;
    clearTimeout(lightTimer);
    pixel = color;
    lightTimer = setTimeout(() => {
      pixel = null;
    }, 1000);
  }
  function chirp() {
    if (!connected || !modules.buzzer) return;
    clearTimeout(chirpTimer);
    chirping = true;
    chirpTimer = setTimeout(() => {
      chirping = false;
    }, 250);
  }
  function fire() {
    light(rule.color);
    if (rule.chirp) chirp();
    note(
      `Rule fired → Pixels ${rule.color}${rule.chirp ? " + a short chirp (visual only)" : ""}. Light switches off after 1 second.`,
    );
  }
  function press(button: string) {
    lastButton = button;
    clearTimeout(buttonTimer);
    buttonTimer = setTimeout(() => {
      lastButton = "";
    }, 400);
    if (button === "C") {
      stop();
      note("Button C stops the demo. All simulated outputs are off.");
    } else if (running && rule.trigger === button) fire();
    else
      note(
        `Button ${button} pressed${running ? " · not this rule's trigger" : " · press Run to use your rule"}.`,
      );
  }
  function move(value: number) {
    const crossed = distance >= rule.threshold && value < rule.threshold;
    distance = value;
    if (running && rule.trigger === "distance" && crossed) fire();
  }
  function run() {
    if (!allowed) return;
    running = true;
    note(
      rule.trigger === "distance"
        ? `Running. Move from above ${rule.threshold} cm to below it to fire the rule.`
        : `Running. Press simulated button ${rule.trigger} to try your rule.`,
    );
  }
  function edit(action: () => void) {
    stop(false);
    action();
    note("Rule changed. Press Run when you're ready to try it.");
  }
  function loadGuardian() {
    stop(false);
    rule = {
      trigger: "distance",
      threshold: 20,
      color: "#ef674f",
      chirp: false,
    };
    mission = true;
    missionOpen = false;
    note(
      "Guardian starter opened. Move a visitor close to change its light. These are your editable rules.",
    );
  }
  function keep() {
    saved = [
      ...saved,
      {
        name: `${mission ? "My guardian" : "My invention"} ${saved.length + 1}`,
        rule: { ...rule },
        mission,
      },
    ];
    note(
      "Kept in this demo only. Reloading the page clears demo saves; real app progress is untouched.",
    );
  }
  async function switchVariant(direction: number) {
    stop(false);
    showState = true;
    const next = variants[(variants.indexOf(variant) + direction + 3) % 3];
    await goto(`?variant=${next}`, {
      replaceState: true,
      noScroll: true,
      keepFocus: true,
    });
  }
  onDestroy(() => {
    stop(false);
    clearTimeout(buttonTimer);
  });
</script>

<svelte:head><title>Tinkeroo · creation prototype</title></svelte:head>

{#snippet moduleControls()}
  <div class="connection-row">
    <div>
      <strong>{connected ? "Demo UNO Q" : "Meet your Board"}</strong><small
        >{connected
          ? `${Object.values(modules).filter(Boolean).length} available module types · simulated`
          : "No account. No passwords. This demo uses pretend hardware."}</small
      >
    </div>
    <button onclick={connect}
      >{connected ? "Disconnect demo" : "Connect demo Board"}</button
    >
  </div>
  <div class="module-list">
    <article class:unavailable={!modules.buttons}>
      <h3>
        ◉ Buttons <small
          >{modules.buttons ? "Available in demo" : "Not connected"}</small
        >
      </h3>
      <p>A and B are inputs. C always stops the demo.</p>
      <div class="row">
        {#each ["A", "B", "C"] as button}<button
            class:pressed={lastButton === button}
            disabled={!connected || !modules.buttons}
            onclick={() => press(button)}>{button}</button
          >{/each}
      </div>
    </article>
    <article class:unavailable={!modules.distance}>
      <h3>
        ↔ Distance <small
          >{modules.distance ? "Available in demo" : "Not connected"}</small
        >
      </h3>
      <label
        >Move a pretend visitor <b
          >{connected && modules.distance ? `${distance} cm` : "—"}</b
        ><input
          aria-label="Simulated visitor distance"
          type="range"
          min="5"
          max="100"
          value={distance}
          disabled={!connected || !modules.distance}
          oninput={(event) => move(Number(event.currentTarget.value))}
        /></label
      >
    </article>
    <article class:unavailable={!modules.pixels}>
      <h3>
        ✦ Pixels <small
          >{modules.pixels ? "Available in demo" : "Not connected"}</small
        >
      </h3>
      <p>Try a color for one second.</p>
      <div class="row">
        <button
          disabled={!connected || !modules.pixels}
          onclick={() => {
            light("#4986ff");
            note("Blue light preview · simulated · one second.");
          }}>Try blue</button
        ><button
          disabled={!connected || !modules.pixels}
          onclick={() => {
            light("#a76be8");
            note("Purple light preview · simulated · one second.");
          }}>Try purple</button
        >
      </div>
    </article>
    <article class:unavailable={!modules.buzzer}>
      <h3>
        ♫ Buzzer <small
          >{modules.buzzer ? "Available in demo" : "Not connected"}</small
        >
      </h3>
      <p>A short chirp. Silent visual preview here.</p>
      <button
        disabled={!connected || !modules.buzzer}
        onclick={() => {
          chirp();
          note(
            "250 ms chirp preview · visual only, no speaker or real Buzzer.",
          );
        }}>Try chirp</button
      >
    </article>
  </div>
  <details>
    <summary>Prototype scenario: change the connected modules</summary>
    <p>
      This changes pretend hardware only. Changing the kit stops the program.
    </p>
    {#each ["buttons", "distance", "pixels", "buzzer"] as name}<label
        class="check"
        ><input
          type="checkbox"
          checked={modules[name as keyof typeof modules]}
          onchange={(event) => {
            stop(false);
            modules[name as keyof typeof modules] = event.currentTarget.checked;
            note(
              "Demo kit changed. Check your rule's required modules, then Run again.",
            );
          }}
        />{name}</label
      >{/each}
  </details>
{/snippet}

{#snippet ruleFields()}
  <label
    >When<select
      value={rule.trigger}
      onchange={(event) =>
        edit(() => {
          rule.trigger = event.currentTarget.value as Trigger;
        })}
      ><option value="A">button A is pressed</option><option value="B"
        >button B is pressed</option
      ><option value="distance">a visitor crosses closer than…</option></select
    ></label
  >
  {#if rule.trigger === "distance"}<label
      >Nearer than <b>{rule.threshold} cm</b><input
        aria-label="Rule distance threshold"
        type="range"
        min="10"
        max="80"
        value={rule.threshold}
        oninput={(event) =>
          edit(() => {
            rule.threshold = Number(event.currentTarget.value);
          })}
      /></label
    >{/if}
  <label class="color-label"
    >Then make Pixels this color <input
      aria-label="Rule light color"
      type="color"
      value={rule.color}
      oninput={(event) =>
        edit(() => {
          rule.color = event.currentTarget.value;
        })}
    /></label
  >
  <label class="check"
    ><input
      type="checkbox"
      checked={rule.chirp}
      onchange={(event) =>
        edit(() => {
          rule.chirp = event.currentTarget.checked;
        })}
    />Also play a short chirp</label
  >
  <p class="muted">
    Outputs are brief, not left on. Editing stops the program.
  </p>
{/snippet}

{#snippet runControls()}
  <div class="row">
    <button class="primary" disabled={!allowed || running} onclick={run}
      >{running ? "● Running" : "▶ Run my rule"}</button
    ><button onclick={keep}>Keep in demo</button>
  </div>
  {#if !allowed}<p class="hint">
      {!connected
        ? "Connect the demo Board to try this rule."
        : "This rule needs Pixels, its chosen input, and Buzzer if chirp is selected. Reconnect the missing demo module or change the rule."}
    </p>{/if}
{/snippet}

{#snippet livePreview()}
  <div class="live-preview">
    <div
      class="invention"
      class:glowing={!!pixel}
      style:--light={pixel ?? "#d9dce9"}
    >
      <div class="eyes">• •</div>
      <div class="lamp"></div>
      <span>{pixel ? "Light on" : "Light off"}</span>
    </div>
    <p class="sound" aria-live="off">
      {chirping ? "♫ Short chirp · visual preview" : "♫ Buzzer off"}
    </p>
    <span class="badge"
      >{running ? "Program running" : "Program stopped"} · simulated</span
    >
  </div>
{/snippet}

{#snippet missionBrief()}
  <div class="brief">
    <span class="eyebrow">A STARTING POINT, NOT A TEST</span>
    <h2>Make a treasure guardian</h2>
    <p>
      Build something that notices a visitor. When they come close, its light
      changes.
    </p>
    <div class="learn">
      <strong>You’ll learn</strong>
      <p>
        Sensor → distance rule → light.<br />Change the distance and predict
        what happens.
      </p>
    </div>
    <p>
      <strong>Start with:</strong> Distance + Pixels.<br />Add buttons and sound
      whenever you want.
    </p>
    <button class="primary" onclick={loadGuardian}>Make my guardian →</button>
    <p class="muted">
      Any materials. One maker or a whole Crew. No names or written answers
      required.
    </p>
  </div>
{/snippet}

{#if dev}
  <div class="prototype-banner">
    DESIGN PROTOTYPE · All hardware is simulated · No Keychain, real Board, or
    persistent saves
  </div>
  <header class="topbar">
    <a class="brand" href="?variant=A"
      ><img src="/icon.svg" alt="" width="34" height="34" />Tinkeroo</a
    >
    <nav aria-label="Prototype activities">
      <button
        class:chosen={!missionOpen}
        onclick={() => {
          missionOpen = false;
        }}>Create</button
      ><button
        class:chosen={missionOpen}
        onclick={() => {
          missionOpen = true;
        }}>Missions</button
      >
    </nav>
    <div class="top-actions">
      <span class="connection-label"
        >{connected ? "Demo Board connected" : "Demo Board not connected"}</span
      ><button class="stop" onclick={() => stop()}>■ Stop outputs</button>
    </div>
  </header>
  <main>
    <section class="feedback" aria-label="What happened">
      <p role="status">{message}</p>
    </section>
    {#if missionOpen}
      <section class="mission-layout">
        {@render missionBrief()}
        <div>
          <div class="mission-sketch">↔ → ◇ → ✦</div>
          <h3>Give it your own purpose</h3>
          <p>
            A dragon’s cave? A spaceship? A quiet visitor greeter? The starter
            is yours to change.
          </p>
          <button
            onclick={() => {
              missionOpen = false;
            }}>I’d rather make my own</button
          >
        </div>
      </section>
    {:else if variant === "A"}
      <div class="studio-layout">
        <section class="studio-main">
          <div class="heading">
            <span class="eyebrow">YOUR INVENTION</span>
            <h1>
              {mission
                ? "My treasure guardian"
                : "What will you bring to life?"}
            </h1>
            <p>
              {mission
                ? "Make its light change when someone approaches. Learn inputs → decisions → outputs."
                : "Try a module. Make one rule. See what happens."}
            </p>
          </div>
          <div class="studio-work">
            <article class="rule-card">
              <span class="eyebrow">ONE LITTLE RULE</span>
              <h2>When this… make that happen.</h2>
              {@render ruleFields()}{@render runControls()}
            </article>
            <section class="preview-card" aria-label="Simulated invention">
              {@render livePreview()}
              <p>Try your input in the Board panel →</p>
            </section>
          </div>
          <button
            class="text-action"
            onclick={() => {
              missionOpen = true;
            }}>Need an idea? Try Treasure Guardian →</button
          >
        </section>
        <aside class="board-panel">
          <h2>Your Board & modules</h2>
          {@render moduleControls()}
        </aside>
      </div>
    {:else if variant === "B"}
      <div class="workbench-heading">
        <span class="eyebrow">HARDWARE WORKBENCH</span>
        <h1>
          {mission
            ? "My guardian program"
            : "Build a behavior, one block at a time."}
        </h1>
        <p>
          Choose a trigger. Choose a response. Run it and watch the connection.
        </p>
      </div>
      <div class="workbench-layout">
        <aside class="palette">
          <h2>Choose blocks</h2>
          <span class="eyebrow">INPUTS</span>{#each ["A", "B"] as button}<button
              class="block input-block"
              onclick={() =>
                edit(() => {
                  rule.trigger = button as Trigger;
                })}>When button {button} is pressed</button
            >{/each}<button
            class="block input-block"
            onclick={() =>
              edit(() => {
                rule.trigger = "distance";
              })}>When a visitor comes close</button
          ><span class="eyebrow">OUTPUTS</span><button
            class="block output-block"
            onclick={() =>
              edit(() => {
                rule.color = "#4986ff";
              })}>Make Pixels blue</button
          ><button
            class="block output-block"
            onclick={() =>
              edit(() => {
                rule.chirp = !rule.chirp;
              })}>{rule.chirp ? "Remove" : "Add"} short chirp</button
          >
          <p class="muted">
            Click to choose blocks. This prototype is not a drag-and-drop
            editor.
          </p>
          <button
            onclick={() => {
              missionOpen = true;
            }}>Open a Mission starter</button
          >
        </aside>
        <section class="program-canvas">
          <h2>Your program</h2>
          <div class="block-stack">
            <div class="block input-block">
              WHEN {rule.trigger === "distance"
                ? `a visitor crosses closer than ${rule.threshold} cm`
                : `button ${rule.trigger} is pressed`}
            </div>
            <div class="block output-block">
              THEN Pixels → <span class="swatch" style:background={rule.color}
              ></span> for 1 second
            </div>
            {#if rule.chirp}<div class="block output-block">
                AND Buzzer → short chirp
              </div>{/if}
          </div>
          <details open>
            <summary>Edit this program</summary>{@render ruleFields()}
          </details>
          {@render runControls()}
        </section>
        <aside class="workbench-board">
          {@render livePreview()}
          <h2>Try your inputs</h2>
          {@render moduleControls()}
        </aside>
      </div>
    {:else}
      <section class="playground-heading">
        <span class="eyebrow">SMALL EXPERIMENTS. BIG IDEAS.</span>
        <h1>First, make something happen.</h1>
        <p>No forms. No right model to build. Start with your curiosity.</p>
      </section>
      <div class="playground-top">
        <section class="playground-modules">
          <h2>1. Meet your modules</h2>
          {@render moduleControls()}
        </section>
        <section class="playground-preview">
          {@render livePreview()}
          <p>This is a pretend invention. Your actual Board stays untouched.</p>
        </section>
      </div>
      <section class="sentence-workspace">
        <div>
          <span class="eyebrow">2. GIVE IT A BEHAVIOR</span>
          <h2>
            {mission ? "A guardian of your own" : "What should happen when…?"}
          </h2>
          <p>Make one connection between an input and an output.</p>
        </div>
        <div class="sentence-editor">
          {@render ruleFields()}{@render runControls()}
        </div>
      </section>
      <button
        class="text-action"
        onclick={() => {
          missionOpen = true;
        }}>Or start with a guardian idea →</button
      >
    {/if}
    {#if trace.length > 1}<details>
        <summary>Recent experiments</summary>{#each trace as line}<p>
            {line}
          </p>{/each}
      </details>{/if}
    {#if saved.length}<section class="saved">
        <h2>Kept in this demo</h2>
        <p>
          Memory only. Reloading clears these; real app saves are never read.
        </p>
        <div class="row">
          {#each saved as item}<button
              onclick={() => {
                stop(false);
                rule = { ...item.rule };
                mission = item.mission;
                missionOpen = false;
                note(
                  `${item.name} reopened. Program is stopped until you press Run.`,
                );
              }}>{item.name} ↗</button
            >{/each}
        </div>
      </section>{/if}
    <details class="state-panel" bind:open={showState}>
      <summary>Prototype state · not a product screen</summary>
      <pre>{JSON.stringify(prototypeSnapshot, null, 2)}</pre>
    </details>
  </main>
  <PrototypeSwitcher
    label={`${variant} · ${names[variants.indexOf(variant)]}`}
    onStep={(direction) => {
      void switchVariant(direction);
    }}
  />
{:else}
  <p>
    This throwaway design prototype is available only in the local development
    server.
  </p>
{/if}

<style>
  :global(body) {
    background: #f7f7fc;
  }
  .prototype-banner {
    padding: 8px 20px;
    background: #26304b;
    color: white;
    text-align: center;
    font-size: 12px;
  }
  .topbar {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 32px;
    padding: 18px 30px;
    background: white;
    border-bottom: 1px solid #dedfec;
  }
  .brand {
    display: flex;
    gap: 10px;
    align-items: center;
    text-decoration: none;
    color: #5842bd;
    font-weight: 850;
    font-size: 23px;
  }
  nav,
  .row,
  .top-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  nav button {
    border: 0;
    background: none;
  }
  nav button.chosen {
    color: #5842bd;
    background: #eee9ff;
  }
  .top-actions {
    margin-left: auto;
  }
  .connection-label {
    font-size: 13px;
    color: #59647c;
  }
  button {
    min-height: 44px;
    border: 1px solid #d7d9e7;
    background: white;
    border-radius: 10px;
    padding: 10px 15px;
    cursor: pointer;
    color: #26304b;
    font: inherit;
  }
  button:hover:not(:disabled) {
    border-color: #5842bd;
    background: #f1edff;
  }
  button:disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }
  .primary {
    color: white;
    background: #5842bd;
    border-color: #5842bd;
    font-weight: 750;
  }
  .primary:hover:not(:disabled) {
    color: white;
    background: #453096;
  }
  .stop {
    border-color: #d99492;
    background: #fff0ee;
    color: #8b2929;
    font-weight: 750;
  }
  main {
    max-width: 1550px;
    margin: auto;
    padding: 30px 30px 130px;
  }
  h1 {
    font-size: clamp(28px, 3vw, 40px);
    line-height: 1.2;
    letter-spacing: -1px;
    margin: 10px 0;
  }
  h2 {
    font-size: 21px;
    margin: 8px 0 18px;
  }
  h3 {
    font-size: 16px;
    margin: 0 0 8px;
  }
  p {
    line-height: 1.6;
  }
  .eyebrow {
    color: #5842bd;
    font-size: 11px;
    font-weight: 850;
    letter-spacing: 0.09em;
  }
  .muted,
  small {
    color: #59647c;
    font-size: 12px;
  }
  small {
    display: block;
    font-weight: 450;
  }
  .studio-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 28px;
  }
  .studio-work {
    display: grid;
    grid-template-columns: minmax(260px, 1fr) minmax(200px, 0.8fr);
    gap: 18px;
    margin-top: 28px;
  }
  .rule-card,
  .preview-card,
  .board-panel,
  .palette,
  .program-canvas,
  .workbench-board,
  .playground-modules,
  .sentence-workspace,
  .brief {
    padding: 24px;
    background: white;
    border: 1px solid #dedfec;
    border-radius: 20px;
  }
  .board-panel {
    align-self: start;
  }
  .connection-row {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 0 18px;
    border-bottom: 1px solid #e5e6ee;
  }
  .module-list article {
    padding: 18px 0;
    border-bottom: 1px solid #e5e6ee;
  }
  .module-list p {
    font-size: 13px;
    margin: 6px 0 12px;
    color: #59647c;
  }
  .module-list h3 small {
    margin-top: 5px;
  }
  .unavailable {
    opacity: 0.65;
  }
  label {
    display: block;
    margin: 15px 0;
    font-size: 14px;
  }
  select {
    display: block;
    width: 100%;
    margin-top: 8px;
    min-height: 44px;
    border: 1px solid #cbd0e2;
    border-radius: 8px;
    padding: 8px;
    font: inherit;
    background: white;
    color: #26304b;
  }
  input[type="range"] {
    display: block;
    width: 100%;
    margin-top: 12px;
    accent-color: #5842bd;
  }
  input[type="color"] {
    display: block;
    width: 72px;
    height: 44px;
    margin-top: 10px;
    padding: 3px;
    border: 1px solid #d7d9e7;
    border-radius: 8px;
  }
  .check {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .check input {
    width: 18px;
    height: 18px;
    accent-color: #5842bd;
  }
  .pressed {
    background: #dcd0ff;
    border-color: #5842bd;
  }
  .live-preview {
    display: grid;
    justify-items: center;
    text-align: center;
    padding: 25px 5px;
  }
  .invention {
    border: 3px solid #d6cbea;
    border-radius: 45px 45px 24px 24px;
    width: 170px;
    height: 185px;
    background: #f0eaff;
    display: grid;
    place-items: center;
    padding: 16px;
  }
  .eyes {
    font-size: 34px;
    color: #5842bd;
    letter-spacing: 14px;
    padding-left: 14px;
  }
  .lamp {
    width: 75px;
    height: 50px;
    border-radius: 14px;
    background: var(--light);
  }
  .glowing .lamp {
    box-shadow: 0 0 26px var(--light);
  }
  .invention span {
    font-size: 12px;
  }
  .sound {
    font-size: 12px;
    min-height: 22px;
  }
  .badge {
    border-radius: 20px;
    padding: 7px 10px;
    background: #eeeaf9;
    color: #5842bd;
    font-size: 12px;
  }
  .text-action {
    margin-top: 24px;
    background: none;
    border: none;
    color: #5842bd;
  }
  .preview-card > p,
  .playground-preview > p {
    text-align: center;
    font-size: 13px;
    color: #59647c;
  }
  .hint {
    background: #fff5dc;
    border-radius: 9px;
    padding: 12px;
    font-size: 13px;
  }
  details {
    margin-top: 18px;
    font-size: 13px;
  }
  summary {
    cursor: pointer;
    font-weight: 700;
    color: #5842bd;
    padding: 8px 0;
  }
  .feedback {
    margin-bottom: 24px;
    border-left: 3px solid #137c73;
    padding: 8px 18px;
    background: #eaf5f3;
    border-radius: 0 12px 12px 0;
  }
  .feedback p {
    margin: 6px 0;
    font-size: 14px;
  }
  .state-panel {
    margin-top: 24px;
  }
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: #eceef6;
    padding: 20px;
    border-radius: 12px;
  }
  .workbench-heading {
    margin-bottom: 24px;
  }
  .workbench-layout {
    display: grid;
    grid-template-columns: 240px minmax(280px, 1fr) 340px;
    gap: 18px;
    align-items: start;
  }
  .palette .eyebrow {
    display: block;
    margin: 18px 0 10px;
  }
  .block {
    display: block;
    text-align: left;
    border-radius: 9px;
    padding: 16px;
    margin-bottom: 8px;
    font-size: 14px;
  }
  .palette .block {
    width: 100%;
  }
  .input-block {
    background: #ece5ff;
    border: 1px solid #c4b2f6;
    color: #453096;
  }
  .output-block {
    background: #e1f2ee;
    border: 1px solid #94c8ba;
    color: #195d4f;
  }
  .block-stack {
    padding: 20px 0;
  }
  .swatch {
    display: inline-block;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    vertical-align: middle;
  }
  .playground-heading {
    text-align: center;
    margin-bottom: 28px;
  }
  .playground-top {
    display: grid;
    grid-template-columns: 1.3fr 0.7fr;
    gap: 28px;
  }
  .playground-modules .module-list {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
  }
  .playground-modules .connection-row {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }
  .playground-preview {
    background: #eee8fc;
    border-radius: 30px;
    padding: 30px;
  }
  .sentence-workspace {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 30px;
    margin-top: 26px;
  }
  .mission-layout {
    display: grid;
    grid-template-columns: minmax(300px, 600px) 1fr;
    gap: 60px;
    max-width: 1050px;
    margin: 20px auto;
    align-items: center;
  }
  .brief h2 {
    font-size: 36px;
    margin-top: 16px;
  }
  .learn {
    border-left: 3px solid #5842bd;
    padding: 10px 20px;
    background: #f5f1ff;
    margin: 22px 0;
  }
  .learn p {
    margin: 8px 0;
  }
  .mission-sketch {
    font-size: 56px;
    color: #5842bd;
    padding: 40px 0;
  }
  .saved {
    margin-top: 24px;
  }
  @media (max-width: 1150px) {
    .studio-work {
      grid-template-columns: 1fr;
    }
    .workbench-layout {
      grid-template-columns: 190px 1fr;
    }
    .workbench-board {
      grid-column: 1 / -1;
    }
    .workbench-board .module-list {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      gap: 15px;
    }
  }
  @media (max-width: 800px) {
    main {
      padding: 20px 16px 140px;
    }
    .topbar {
      padding: 14px;
      flex-wrap: wrap;
      gap: 12px;
    }
    .top-actions {
      margin-left: 0;
      width: 100%;
      justify-content: space-between;
    }
    .studio-layout,
    .workbench-layout,
    .playground-top,
    .sentence-workspace,
    .mission-layout {
      grid-template-columns: 1fr;
    }
    .workbench-board .module-list,
    .playground-modules .module-list {
      grid-template-columns: 1fr;
    }
  }
</style>
