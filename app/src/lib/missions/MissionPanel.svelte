<script lang="ts">
  import { onMount } from "svelte";
  import type { BoardClient, BoardSnapshot } from "../board/types";
  import type { SimulatorBoardClient } from "../board/simulator";
  import { treasureGuardian as mission } from "./catalog";
  import { tracks, type Track } from "./definition";
  import { MissionSession, type MissionProgress } from "./session";
  import { ProgressRepository } from "./storage";
  import { GuardianRuntime } from "./guardian";

  let {
    board,
    simulation,
  }: { board: BoardClient; simulation?: SimulatorBoardClient["controls"] } =
    $props();
  const repository = new ProgressRepository();
  let session: MissionSession;
  let runtime: GuardianRuntime;
  let progress = $state<MissionProgress | null>(null);
  let live = $state<ReturnType<GuardianRuntime["getStatus"]> | null>(null);
  let snapshot = $state<BoardSnapshot | null>(null);
  let error = $state("");
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

  onMount(() => {
    let disposed = false;
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
        await runtime.start();
        ready = true;
        await refreshLive();
        if (disposed) return;
        timer = setInterval(() => {
          if (polling || busy || progress?.completion) return;
          polling = true;
          void refreshLive()
            .catch((cause) => {
              error = String(cause);
            })
            .finally(() => {
              polling = false;
            });
        }, 250);
      } catch (cause) {
        error = `Could not open this Mission: ${String(cause)}. Existing saves have not been replaced.`;
      }
    })();
    return () => {
      disposed = true;
      clearInterval(timer);
      void initialization
        .then(() => runtime?.stop())
        .catch(() => {
          /* Board watchdog clears outputs if unavailable. */
        });
    };
  });

  async function act(action: () => void | Promise<void>) {
    if (!progress || busy) return;
    busy = true;
    error = "";
    try {
      await action();
      progress = session.getProgress();
      status = "Saving…";
      await repository.save(progress);
      status = "Progress saved on this Mac only.";
      if (progress.completion) {
        ready = false;
        await runtime.stop();
      }
    } catch (cause) {
      error = String(cause);
      status =
        "Check the message above. Use Save again to retry an unsaved change.";
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

<section class="mission">
  <p class="eyebrow">{mission.world} / MISSION 05</p>
  <h2>{mission.title}</h2>
  <p>{mission.narrative}</p>
  <p role="status">{status}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if progress && stage}
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
    {#if progress.completion}
      <article>
        <h3>Mission complete — your invention, your rules!</h3>
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
            <button class="primary" onclick={() => act(continueMission)}
              >Check and continue →</button
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
      <fieldset disabled={!ready || busy}>
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
    <button disabled={busy} onclick={() => act(() => {})}>Save again</button>
  {/if}
</section>

<style>
  .mission {
    margin-top: 30px;
    max-width: 1100px;
  }
  .eyebrow {
    color: #8c531b;
    font-weight: 800;
    letter-spacing: 0.1em;
  }
  h2 {
    font-family: Georgia, serif;
    font-size: 42px;
    margin: 12px 0;
  }
  h3 {
    font-size: 24px;
  }
  fieldset {
    border: 1px solid #d9d2c3;
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
    border: 1px solid #aaa;
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
    border: 1px solid #b5b7a5;
    border-radius: 8px;
    background: #fffdf5;
    color: #18302b;
    font: inherit;
    cursor: pointer;
  }
  .primary {
    background: #173b35;
    color: white;
  }
  article {
    padding: 24px;
    background: #fffcf5;
    border-radius: 16px;
  }
  .error {
    color: #9d2121;
    padding: 15px;
    background: #ffe6df;
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
    border: 2px solid #173b35;
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
    background: #173b35;
    color: white;
    padding: 15px;
  }
</style>
