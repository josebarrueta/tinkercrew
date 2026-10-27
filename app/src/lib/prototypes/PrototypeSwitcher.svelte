<script lang="ts">
  import { dev } from "$app/environment";
  let {
    label,
    onStep,
  }: { label: string; onStep: (direction: number) => void } = $props();
  function keys(event: KeyboardEvent) {
    if (
      event.target instanceof Element &&
      event.target.closest("input, textarea, select, button, [contenteditable]")
    )
      return;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      onStep(event.key === "ArrowLeft" ? -1 : 1);
    }
  }
</script>

<svelte:window onkeydown={keys} />
{#if dev}
  <div class="switcher" aria-label="Prototype layout switcher">
    <button aria-label="Previous layout" onclick={() => onStep(-1)}>←</button>
    <span><small>COMPARE LAYOUTS</small><strong>{label}</strong></span>
    <button aria-label="Next layout" onclick={() => onStep(1)}>→</button>
  </div>
{/if}

<style>
  .switcher {
    position: fixed;
    bottom: 18px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 16px;
    border-radius: 18px;
    color: white;
    background: #26304b;
    padding: 10px 14px;
    box-shadow: 0 8px 25px #26304b33;
    z-index: 10;
    max-width: calc(100vw - 24px);
  }
  button {
    min-width: 44px;
    min-height: 44px;
    border-radius: 10px;
    font: inherit;
    background: #404963;
    color: white;
    border: 0;
    cursor: pointer;
  }
  small {
    display: block;
    color: #d9ddef;
    font-size: 9px;
    letter-spacing: 0.1em;
  }
  strong {
    font-size: 13px;
  }
  @media (max-width: 800px) {
    .switcher {
      width: max-content;
      gap: 8px;
    }
  }
</style>
