<script lang="ts" generics="T extends string">
  let { label, hint = "", value, options, disabled = false, onchange }: {
    label: string;
    hint?: string;
    value: T;
    options: { id: T; label: string }[];
    disabled?: boolean;
    onchange: (value: T) => void;
  } = $props();
</script>

<div class="row" class:disabled>
  <span class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <div class="seg" role="radiogroup" aria-label={label}>
    {#each options as o (o.id)}
      <button role="radio" aria-checked={value === o.id} {disabled} onclick={() => onchange(o.id)}>{o.label}</button>
    {/each}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }
  :global(.row) + .row { border-top: 1px solid var(--hairline); }
  .disabled { opacity: 0.5; }
  .text { display: flex; flex-direction: column; gap: 2px; }
  .label { font-weight: 550; }
  .hint { color: var(--muted); font-size: 12.5px; }
  .seg {
    flex: none;
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface-2, var(--surface-1));
    border: 1px solid var(--hairline);
  }
  button {
    font: inherit;
    font-size: 12.5px;
    padding: 5px 11px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    transition: background 140ms, color 140ms;
  }
  button[aria-checked="true"] { background: var(--brand, var(--accent)); color: #fff; box-shadow: 0 2px 8px -2px color-mix(in srgb, var(--accent) 60%, transparent); }
  button:disabled { cursor: default; }
</style>
