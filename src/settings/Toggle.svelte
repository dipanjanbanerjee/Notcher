<script lang="ts">
  let { checked, label, hint = "", onchange }: {
    checked: boolean;
    label: string;
    hint?: string;
    onchange: (value: boolean) => void;
  } = $props();
</script>

<label class="row">
  <span class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <input type="checkbox" role="switch" {checked} onchange={(e) => onchange(e.currentTarget.checked)} />
</label>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
    cursor: pointer;
  }
  .row + :global(.row) { border-top: 1px solid var(--hairline); }
  .text { display: flex; flex-direction: column; gap: 2px; }
  .label { font-weight: 550; }
  .hint { color: var(--muted); font-size: 12.5px; }
  input {
    appearance: none;
    flex: none;
    width: 40px;
    height: 24px;
    border-radius: 12px;
    /* The brand gradient sits underneath and the "off" colour is a thick inner
       shadow over it, so turning on fills in smoothly instead of snapping. */
    background: var(--brand, var(--accent));
    box-shadow: inset 0 0 0 12px color-mix(in srgb, var(--fg) 13%, var(--window-bg, var(--bg)));
    position: relative;
    cursor: pointer;
    transition: box-shadow 320ms cubic-bezier(0.2, 0.9, 0.25, 1);
  }
  input::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.3);
    transition: transform 260ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  input:checked { box-shadow: inset 0 0 0 0 transparent; }
  input:checked::after { transform: translateX(16px); }
  input:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
