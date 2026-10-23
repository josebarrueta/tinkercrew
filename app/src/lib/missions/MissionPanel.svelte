<script lang="ts">
  import { onMount } from "svelte";
  import { treasureGuardian as mission } from "./catalog";
  import { tracks, type Track } from "./definition";
  import { MissionSession, type MissionProgress } from "./session";
  import { ProgressRepository } from "./storage";

  const repository = new ProgressRepository();
  let session: MissionSession;
  let progress = $state<MissionProgress | null>(null);
  let error = $state("");
  let status = $state("Loading local progress…");
  let busy = $state(false);
  const stage = $derived(progress ? mission.stages[progress.stageIndex] : null);
  const hintLevel = $derived(
    progress && stage ? (progress.hints[stage.id] ?? 0) : 0,
  );
  let remix = $state("");
  let changed = $state("");
  let failed = $state("");

  onMount(() => {
    void (async () => {
      try {
        session = new MissionSession(
          mission,
          await repository.load(mission.id),
        );
        progress = session.getProgress();
        remix = progress.remix ?? "";
        changed = progress.reflection?.changed ?? "";
        failed = progress.reflection?.failed ?? "";
        status = "Progress saved on this Mac only.";
      } catch (cause) {
        error = `Could not resume this Mission: ${String(cause)}. Your save has not been replaced.`;
      }
    })();
  });

  async function act(action: () => void) {
    if (!progress || busy) return;
    busy = true;
    error = "";
    try {
      action();
      progress = session.getProgress();
      status = "Saving…";
      await repository.save(progress);
      status = "Progress saved on this Mac only.";
    } catch (cause) {
      error = String(cause);
      status = "Not saved yet. Retry saving before closing the app.";
    } finally {
      busy = false;
    }
  }

  function updateMember(index: number, name: string, track: Track) {
    const members = session.getProgress().crew;
    members[index] = { ...members[index], name, track };
    session.setCrew(members);
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
                act(() => {
                  session.confirm();
                  session.advance();
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
                act(() => {
                  session.recordRemix(remix);
                  session.advance();
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
                act(() => {
                  session.reflect(changed, failed);
                  session.advance();
                })}>Finish our Mission →</button
            >
          {:else}
            <p>
              Live check: {stage.condition.check}. Connect a Mission runtime to
              collect fresh evidence.
            </p>
            <button class="primary" onclick={() => act(() => session.advance())}
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
</style>
