<script lang="ts">
  import { onMount } from "svelte";
  import { errorMessage } from "../errors";
  import type { BoardClient, BoardSnapshot } from "../board/types";
  import type { SimulatorBoardClient } from "../board/simulator";
  import { treasureGuardian as mission } from "./catalog";
  import { tracks, type Track } from "./definition";
  import { MissionSession, type MissionProgress } from "./session";
  import { ProgressRepository } from "./storage";
  import { GuardianRuntime } from "./guardian";
  import HardwareSafety from "../board/HardwareSafety.svelte";

  let {
    board,
    simulation,
    onSetup = () => {},
    onHome = () => {},
  }: {
    board: BoardClient;
    simulation?: SimulatorBoardClient["controls"];
    onSetup?: () => void;
    onHome?: () => void;
  } = $props();
  const repository = new ProgressRepository();
  let session: MissionSession;
  let runtime: GuardianRuntime;
  let progress = $state<MissionProgress | null>(null);
  let live = $state<ReturnType<GuardianRuntime["getStatus"]> | null>(null);
  let snapshot = $state<BoardSnapshot | null>(null);
  let error = $state("");
  let hardwareError = $state("");
  let disposed = false;
  let controlsStarted = false;
  let controlsTask: Promise<void> = Promise.resolve();
  let status = $state("Loading local progress…");
  let busy = $state(false);
  let polling = false;
  let ready = $state(false);
  const stage = $derived(progress ? mission.stages[progress.stageIndex] : null);
  const hintLevel = $derived(
    progress && stage ? (progress.hints[stage.id] ?? 0) : 0,
  );
  let remix = $state("");
  let changed = $state("");
  let failed = $state("");
  let lights = $state(false);
  let sound = $state(false);
  let intruderMm = $state(150);
  let warningMm = $state(350);
  let warningColor = $state("#ffb703");
  let alarmMs = $state(350);
  let ruleText = $state("");

  async function refreshLive() {
    await runtime.tick();
    live = runtime.getStatus();
    snapshot = await board.getSnapshot();
    for (const [check, passed] of Object.entries(live.evidence)) {
      session.observe(check as keyof typeof live.evidence, passed);
    }
  }

  function enableMissionControls(): Promise<void> {
    if (!progress || busy || disposed) return Promise.resolve();
    busy = true;
    hardwareError = "";
    controlsStarted = true;
    controlsTask = (async () => {
      try {
        await runtime.start();
        if (disposed) return;
        await refreshLive();
        if (live?.error) throw new Error(live.error);
        ready = true;
      } catch (cause) {
        ready = false;
        hardwareError = errorMessage(
          cause,
          "Reconnect your Board in Board Setup, then enable Mission controls again.",
        );
        await runtime.stop().catch(() => {});
        controlsStarted = false;
      } finally {
        busy = false;
      }
    })();
    return controlsTask;
  }

  async function stopMissionControls() {
    if (busy) return;
    busy = true;
    try {
      await runtime.stop();
      live = runtime.getStatus();
    } catch (cause) {
      hardwareError = errorMessage(cause);
    } finally {
      ready = false;
      controlsStarted = false;
      busy = false;
    }
  }

  onMount(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const initialization = (async () => {
      try {
        const saved = await repository.load(mission.id);
        if (disposed) return;
        session = new MissionSession(mission, saved);
        progress = session.getProgress();
        remix = progress.remix ?? "";
        changed = progress.reflection?.changed ?? "";
        failed = progress.reflection?.failed ?? "";
        ({ intruderMm, warningMm, warningColor, alarmMs } =
          progress.guardianRules);
        ruleText = JSON.stringify(progress.guardianRules, null, 2);
        status = "Progress saved on this Mac only.";
        runtime = new GuardianRuntime(board, progress.guardianRules);
        if (progress.completion) return;
        if (simulation) {
          await enableMissionControls();
        } else {
          // Opening a story is not consent to a physical output lease.
          try {
            snapshot = await board.getSnapshot();
          } catch (cause) {
            hardwareError = errorMessage(cause);
          }
        }
        if (disposed) return;
        timer = setInterval(() => {
          if (!ready || polling || busy || progress?.completion) return;
          polling = true;
          void refreshLive()
            .catch(async (cause) => {
              hardwareError = errorMessage(cause);
              ready = false;
              controlsStarted = false;
              await runtime.stop().catch(() => {});
              live = runtime.getStatus();
            })
            .finally(() => {
              polling = false;
            });
        }, 250);
      } catch (cause) {
        error = `Could not open this Mission: ${errorMessage(cause)}. Existing saves have not been replaced.`;
      }
    })();
    return () => {
      disposed = true;
      clearInterval(timer);
      void Promise.all([initialization, controlsTask])
        .then(() => (controlsStarted ? runtime?.stop() : undefined))
        .catch(() => {
          /* Board watchdog clears outputs if unavailable. */
        });
    };
  });

  async function act(action: () => void | Promise<void>) {
    if (!progress || busy) return;
    busy = true;
    error = "";
    let changedProgress = false;
    try {
      await action();
      changedProgress = true;
      progress = session.getProgress();
      status = "Saving…";
      await repository.save(progress);
      status = "Progress saved on this Mac only.";
      if (progress.completion) {
        ready = false;
        if (controlsStarted) {
          await runtime.stop();
          controlsStarted = false;
        }
      }
    } catch (cause) {
      error = errorMessage(cause);
      status = changedProgress
        ? "Your latest change is not saved yet. Use Save again to retry; existing saves have not been replaced."
        : "This step was not completed. Follow the message and try again; existing saves have not been replaced.";
    } finally {
      busy = false;
    }
  }

  async function continueMission() {
    if (session.stage.condition.kind === "observable") await refreshLive();
    session.advance();
    if (session.stage.id === "checks") runtime.resetChallenge();
  }

  function updateMember(index: number, name: string, track: Track) {
    const members = session.getProgress().crew;
    members[index] = { ...members[index], name, track };
    session.setCrew(members);
  }

  async function applyRules(input: unknown) {
    runtime.updateRules(input);
    session.setGuardianRules(runtime.getStatus().rules);
    const rules = runtime.getStatus().rules;
    ({ intruderMm, warningMm, warningColor, alarmMs } = rules);
    ruleText = JSON.stringify(rules, null, 2);
    await refreshLive();
  }
</script>

{#snippet missionControls()}
  {#if progress && stage && !progress.completion}
    <section class="control-status" aria-label="Mission Board connection">
      <p>
        <strong
          >{simulation
            ? "Practice simulator — no physical outputs"
            : "Connected Board"}</strong
        >{#if !ready}
          · <strong>Mission controls are off.</strong>{/if}
      </p>
      <details
        open={stage.condition.kind === "observable" ||
          stage.condition.kind === "remix" ||
          !!hardwareError}
      >
        <summary>Mission hardware controls (lights &amp; sound)</summary>
        {#if !ready}
          <p>
            Your story and saved progress are available. Enable controls only
            when the connected components are ready for lights and sound.
            Reconnecting never reenables them automatically.
          </p>
          <button disabled={busy} onclick={enableMissionControls}
            >Enable Mission controls</button
          >
        {:else}
          <p>
            Mission controls enabled. Press A to arm the guardian, B to silence,
            C to disarm.
          </p>
          <button disabled={busy} onclick={stopMissionControls}
            >Stop Mission controls</button
          >
        {/if}
      </details>
      {#if hardwareError}<p role="alert" class="error">{hardwareError}</p>
        <button onclick={onSetup}>Reconnect in Board Setup</button>{/if}
    </section>
  {/if}
{/snippet}

<section class="mission">
  <div class="mission-navigation">
    <button onclick={onHome}>Back to Home Base</button>
    <button onclick={onSetup}>Open Board Setup</button>
  </div>
  <h2>{mission.title}</h2>
  <details class="mission-intro">
    <summary>About this Mission · {mission.world}</summary>
    <p>{mission.narrative}</p>
  </details>
  <p role="status">{status}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if stage && stage.condition.kind !== "human"}{@render missionControls()}{/if}
  {#if progress && stage}
    <progress
      aria-label="Mission progress"
      max={mission.stages.length}
      value={progress.completion ? mission.stages.length : progress.stageIndex}
    ></progress>
    <details class="crew-details" open={stage.id === "roles"}>
      <summary>Crew roles & challenge tracks</summary>
      <fieldset disabled={busy}>
        <legend>Your shared Crew</legend>
        <div class="crew-grid">
          {#each progress.crew as member, index}
            <div>
              <label
                >Maker {index + 1}
                <input
                  aria-label={`Maker ${index + 1} name`}
                  maxlength="60"
                  value={member.name}
                  onchange={(event) =>
                    act(() =>
                      updateMember(
                        index,
                        event.currentTarget.value,
                        member.track,
                      ),
                    )}
                />
              </label>
              <strong>{member.role}</strong>
              <label
                >Challenge track
                <select
                  value={member.track}
                  onchange={(event) =>
                    act(() =>
                      updateMember(
                        index,
                        member.name,
                        event.currentTarget.value as Track,
                      ),
                    )}
                >
                  {#each tracks as track}<option>{track}</option>{/each}
                </select>
              </label>
              <p>{mission.tracks[member.track]}</p>
            </div>
          {/each}
        </div>
        <button onclick={() => act(() => session.rotateRoles())}
          >Rotate roles ↻</button
        >
      </fieldset>
    </details>
    {#if progress.completion}
      <article>
        <h3>Mission complete — your invention, your rules!</h3>
        <p>
          This is your Crew's saved Mission progress, not proof of physical
          Board acceptance. Simulator practice does not verify connected
          hardware.
        </p>
        <p>{progress.remix}</p>
        <p>Changed: {progress.reflection?.changed}</p>
        <p>Surprised us: {progress.reflection?.failed}</p>
        <h3>Free Build invitations</h3>
        {#each mission.freeBuild as invitation}
          <p>
            <strong>{invitation.prompt}</strong> — {invitation.components.join(
              ", ",
            )}. {invitation.constraint}.
          </p>
        {/each}
      </article>
    {:else}
      <article>
        <p>Stage {progress.stageIndex + 1} of {mission.stages.length}</p>
        <h3>{stage.title}</h3>
        <p>{stage.goal}</p>
        {#if stage.id === "build"}<details>
            <summary>Wiring safety details</summary><HardwareSafety />
          </details>{/if}
        <p>
          Required: {mission.components.required.join(", ")}. Optional: {mission.components.optional.join(
            ", ",
          )}.
        </p>
        <fieldset disabled={busy}>
          {#if stage.condition.kind === "human"}
            <button
              class="primary"
              onclick={() =>
                act(async () => {
                  session.confirm();
                  await continueMission();
                })}>Our Crew confirms — continue →</button
            >
          {:else if stage.condition.kind === "remix"}
            <label
              >What did you change and test?<textarea
                maxlength="2000"
                bind:value={remix}></textarea></label
            >
            <button
              class="primary"
              onclick={() =>
                act(async () => {
                  await refreshLive();
                  if (!runtime.getStatus().testedRemix)
                    throw new Error(
                      "Change a rule below, arm the guardian, and test it before recording your remix.",
                    );
                  session.recordRemix(remix);
                  await continueMission();
                })}>Record our remix →</button
            >
          {:else if stage.condition.kind === "reflection"}
            <label
              >What changed?<textarea maxlength="2000" bind:value={changed}
              ></textarea></label
            >
            <label
              >What failed or surprised you?<textarea
                maxlength="2000"
                bind:value={failed}></textarea></label
            >
            <button
              class="primary"
              onclick={() =>
                act(async () => {
                  session.reflect(changed, failed);
                  await continueMission();
                })}>Finish our Mission →</button
            >
          {:else}
            <p>
              Live check: {stage.condition.check} — {live?.evidence[
                stage.condition.check
              ]
                ? "passed"
                : "keep experimenting; retry any time"}.
            </p>
            {#if stage.condition.check === "self-test"}
              <button
                disabled={!ready}
                onclick={() =>
                  act(async () => {
                    lights = false;
                    sound = false;
                    await runtime.beginSelfTest();
                    await refreshLive();
                  })}>Run hardware self-test</button
              >
              <label class="check"
                ><input type="checkbox" bind:checked={lights} />We saw the blue
                light</label
              >
              <label class="check"
                ><input type="checkbox" bind:checked={sound} />We heard the
                short tone (or saw the simulator buzzer output)</label
              >
              <button
                disabled={!ready}
                onclick={() =>
                  act(async () => {
                    await runtime.confirmSelfTest(lights, sound);
                    await refreshLive();
                  })}>Confirm light and sound</button
              >
            {/if}
            {#if stage.condition.check === "behavior"}
              <button onclick={() => act(() => runtime.resetChallenge())}
                >Restart behavior challenge</button
              >
            {/if}
            <button
              class="primary"
              disabled={!ready}
              onclick={() => act(continueMission)}>Check and continue →</button
            >
          {/if}
          <button
            onclick={() =>
              act(() => {
                session.revealHint();
              })}>Hint {Math.min(3, hintLevel + 1)} of 3</button
          >
          {#each stage.hints.slice(0, hintLevel) as hint, index}<p>
              <strong>Hint {index + 1}:</strong>
              {hint}
            </p>{/each}
        </fieldset>
      </article>
      {#if stage.condition.kind === "human"}{@render missionControls()}{/if}
      {#if ready}
        <fieldset disabled={busy}>
          <legend>Live guardian — {snapshot?.mode ?? "Board"}</legend>
          <p aria-live="polite">
            <strong data-testid="guardian-state"
              >{live?.state ?? "starting"}</strong
            >
            · {snapshot?.observables.distanceMillimeters ?? "—"} mm · {live?.silenced
              ? "silenced"
              : "sound enabled"}
          </p>
          {#if live?.error}<p class="error" role="alert">{live.error}</p>{/if}
          <div class="outputs">
            <span
              class="pixel"
              style:background={snapshot?.outputs.pixels?.color ?? "#777"}
            ></span>
            <span>Pixels: {snapshot?.outputs.pixels?.color ?? "off"}</span>
            <span
              >Buzzer: {snapshot?.outputs.buzzer
                ? `${snapshot.outputs.buzzer.frequencyHz} Hz / ${snapshot.outputs.buzzer.durationMs} ms`
                : "off"}</span
            >
          </div>
          {#if simulation}
            <label
              >Simulated distance (mm)<input
                type="range"
                min="20"
                max="1000"
                step="10"
                value={snapshot?.observables.distanceMillimeters ?? 600}
                oninput={(event) => {
                  simulation?.setDistance(Number(event.currentTarget.value));
                }}
              /></label
            >
            {#each ["A — Arm", "B — Silence", "C — Disarm"] as label, index}
              <button onclick={() => simulation?.pressButton(index)}
                >{label}</button
              >
            {/each}
            <label class="check"
              ><input
                type="checkbox"
                onchange={(event) =>
                  simulation?.freezeDistance(event.currentTarget.checked)}
              />Freeze distance readings (safety experiment)</label
            >
          {:else}
            <p>
              Move an object in front of the distance sensor and use the Board's
              A, B, and C buttons.
            </p>
          {/if}
          <details>
            <summary>Inventor — adjust the rules</summary>
            <div class="crew-grid">
              <label
                >Intruder boundary (mm)<input
                  type="number"
                  min="20"
                  max="3800"
                  bind:value={intruderMm}
                /></label
              >
              <label
                >Warning boundary (mm)<input
                  type="number"
                  min="40"
                  max="4000"
                  bind:value={warningMm}
                /></label
              >
              <label
                >Warning color<input
                  type="color"
                  bind:value={warningColor}
                /></label
              >
              <label
                >Alarm duration (ms)<input
                  type="number"
                  min="100"
                  max="1000"
                  bind:value={alarmMs}
                /></label
              >
            </div>
            <button
              onclick={() =>
                act(() =>
                  applyRules({
                    ...runtime.getStatus().rules,
                    intruderMm,
                    warningMm,
                    warningColor,
                    alarmMs,
                  }),
                )}>Apply and test rules</button
            >
          </details>
          <details>
            <summary>Hacker — bounded rule configuration & event log</summary>
            <p>
              This JSON is data, never executable code. Only the six displayed
              rules are accepted.
            </p>
            <label
              >Guardian rule JSON<textarea rows="10" bind:value={ruleText}
              ></textarea></label
            >
            <button onclick={() => act(() => applyRules(JSON.parse(ruleText)))}
              >Validate and apply JSON</button
            >
            <pre>{live?.log.join("\n") ?? "Waiting for input events…"}</pre>
          </details>
          <details>
            <summary>Live evidence</summary>
            <pre>{JSON.stringify(live?.evidence, null, 2)}</pre>
          </details>
        </fieldset>
      {/if}
    {/if}
    <button disabled={busy} onclick={() => act(() => {})}>Save again</button>
  {/if}
</section>

<style>
  .mission-navigation {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .mission-intro {
    margin-bottom: 12px;
  }
  .control-status {
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: 12px;
    margin-bottom: 16px;
  }
  .control-status p {
    margin: 8px 0;
  }
  .mission {
    margin-top: 20px;
    max-width: 1100px;
  }
  h2 {
    font-size: 38px;
    line-height: 1.2;
    letter-spacing: -1px;
    margin: 12px 0;
  }
  h3 {
    font-size: 24px;
    margin: 8px 0 12px;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 18px;
    margin: 16px 0;
  }
  fieldset:disabled {
    opacity: 0.65;
  }
  .crew-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 18px;
  }
  label,
  strong {
    display: block;
    margin-bottom: 8px;
  }
  input,
  select,
  textarea {
    display: block;
    width: 100%;
    padding: 10px;
    margin: 6px 0 12px;
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .check input {
    width: auto;
    margin: 0;
  }
  textarea {
    min-height: 75px;
  }
  button {
    padding: 10px 15px;
    margin: 6px 8px 6px 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: var(--primary);
    font: inherit;
    cursor: pointer;
  }
  .primary {
    background: var(--primary);
    color: white;
    font-weight: 800;
  }
  .primary:hover {
    background: var(--primary-hover);
  }
  article {
    padding: 24px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 18px;
    box-shadow: var(--shadow);
  }
  .error {
    color: var(--danger);
    padding: 15px;
    background: var(--danger-bg);
    border-radius: 10px;
  }
  .outputs {
    display: flex;
    gap: 14px;
    align-items: center;
    margin: 15px 0;
  }
  .pixel {
    display: inline-block;
    width: 36px;
    height: 36px;
    border: 2px solid var(--ink);
    border-radius: 50%;
  }
  details {
    margin: 16px 0;
  }
  summary {
    cursor: pointer;
    font-weight: bold;
  }
  pre {
    overflow: auto;
    max-height: 220px;
    font-size: 12px;
    background: var(--code-bg);
    color: var(--code-ink);
    border-radius: 10px;
    padding: 15px;
  }
</style>
