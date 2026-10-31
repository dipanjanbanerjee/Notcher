<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import Picker, { type PickerOption } from "../../components/Picker.svelte";
  import { CATEGORIES, DEFAULT_PAIR, POPULAR_CURRENCIES, UNITS, convert, formatNumber, symbol, type Category } from "../../lib/units";
  import { rates } from "../../lib/stores/weather.svelte";

  const KEY = "notcher.convert";
  type Saved = { category: Category; from: string; to: string; input: string };
  function restore(): Saved {
    try {
      const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as Saved | null;
      if (s && CATEGORIES.some((c) => c.id === s.category)) return s;
    } catch {
      /* start fresh */
    }
    return { category: "length", from: "km", to: "mi", input: "1" };
  }
  const saved = restore();
  let category = $state<Category>(saved.category);
  let from = $state(saved.from);
  let to = $state(saved.to);
  let input = $state(saved.input);
  $effect(() => {
    try {
      localStorage.setItem(KEY, JSON.stringify({ category, from, to, input }));
    } catch {
      /* not saved */
    }
  });

  $effect(() => {
    if (category === "currency") void rates.ensure();
  });

  const currencyName = (() => {
    try {
      const names = new Intl.DisplayNames(["en"], { type: "currency" });
      return (code: string) => names.of(code) ?? code;
    } catch {
      return (code: string) => code;
    }
  })();

  const options = $derived.by((): PickerOption[] => {
    if (category !== "currency") return UNITS[category].map((u) => ({ id: u.id, label: u.label, hint: symbol(u.id) }));
    const codes = Object.keys(rates.current?.rates ?? {});
    const popular = POPULAR_CURRENCIES.filter((c) => !codes.length || codes.includes(c));
    const rest = codes.filter((c) => !POPULAR_CURRENCIES.includes(c)).sort();
    return [...popular, ...rest].map((c) => ({ id: c, label: currencyName(c), hint: c }));
  });

  function pick(next: Category) {
    if (next === category) return;
    category = next;
    [from, to] = DEFAULT_PAIR[next];
    if (next === "temperature" && input === "1") input = "25";
  }

  const parse = (s: string) => Number(s.replace(/[,\s]/g, ""));
  const conv = (v: number) => convert(category, v, from, to, rates.current?.rates);
  const result = $derived(input.trim() === "" ? null : conv(parse(input)));
  const plain = (n: number | null) => formatNumber(n).replace(/,/g, "");
  const unitText = (id: string) => (category === "currency" ? id : symbol(id));

  /** A few handy amounts, so a quick look covers more than one number. */
  const SAMPLES: Record<string, number[]> = { temperature: [-10, 0, 20, 37, 100], default: [1, 5, 10, 50, 100] };
  const samples = $derived((SAMPLES[category] ?? SAMPLES.default).map((v) => ({ v, r: conv(v) })));

  function swap() {
    [from, to] = [to, from];
    if (result !== null) input = plain(result);
  }

  let copied = $state(false);
  async function copy() {
    if (result === null) return;
    try {
      await navigator.clipboard.writeText(plain(result));
      copied = true;
      setTimeout(() => (copied = false), 1200);
    } catch {
      /* clipboard unavailable */
    }
  }

  let chipRow: HTMLDivElement;
  function wheel(e: WheelEvent) {
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      chipRow.scrollBy({ left: e.deltaY, behavior: "smooth" });
      e.preventDefault();
    }
  }
  $effect(() => {
    chipRow?.querySelector<HTMLElement>(`[data-id="${category}"]`)?.scrollIntoView({ inline: "nearest", block: "nearest" });
  });
</script>

<div class="convert">
  <div class="chips" bind:this={chipRow} onwheel={wheel} role="tablist" aria-label="What to convert">
    {#each CATEGORIES as c (c.id)}
      <button role="tab" data-id={c.id} aria-selected={category === c.id} onclick={() => pick(c.id)}>{c.label}</button>
    {/each}
  </div>

  <div class="pair">
    <div class="side">
      <span class="tag">From</span>
      <input type="text" inputmode="decimal" spellcheck="false" aria-label="Amount" bind:value={input} />
      <Picker label="From" {options} value={from} onchange={(id) => (from = id)} searchable={category === "currency"} sheet="left" />
    </div>
    <button class="swap" aria-label="Swap units" title="Swap" onclick={swap}><Icon name="swap" size={16} /></button>
    <div class="side out">
      <span class="tag">To{copied ? " · copied" : ""}</span>
      <button class="result" title="Click to copy" onclick={copy}>
        {#key result}<span>{formatNumber(result)}</span>{/key}
      </button>
      <Picker label="To" {options} value={to} onchange={(id) => (to = id)} searchable={category === "currency"} sheet="right" />
    </div>
  </div>

  <div class="foot">
    {#if category === "currency" && !rates.current}
      <span class="muted">{rates.error ?? "Fetching today's rates…"}</span>
    {:else}
      {#each samples as s (s.v)}
        <button class="sample" onclick={() => (input = String(s.v))} title="Use {s.v}">
          <b>{formatNumber(s.v)} {unitText(from)}</b><span>{formatNumber(s.r)} {unitText(to)}</span>
        </button>
      {/each}
    {/if}
  </div>
  {#if category === "currency" && rates.current}
    <p class="note">Rates updated {new Date(rates.current.updated).toLocaleDateString("en-US", { day: "numeric", month: "short" })}{rates.error ? ` · ${rates.error}` : ""}</p>
  {/if}
</div>

<style>
  .convert { position: relative; height: 100%; display: flex; flex-direction: column; gap: 12px; }
  .muted { color: var(--muted); font-size: 12px; }
  .chips {
    display: flex; gap: 4px; overflow-x: auto; scrollbar-width: none; flex: none;
    padding: 3px; border-radius: 12px; background: var(--surface-1);
  }
  .chips::-webkit-scrollbar { display: none; }
  .chips button {
    flex: 1 0 auto; border: 0; padding: 5px 10px; border-radius: 9px;
    background: transparent; color: var(--muted); font: inherit; font-size: 12px; cursor: pointer;
    transition: background 220ms, color 200ms;
  }
  .chips button:hover { color: var(--fg); }
  .chips button[aria-selected="true"] { background: var(--surface-2); color: var(--fg); }
  .pair { display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; gap: 10px; }
  .side {
    display: flex; flex-direction: column; gap: 6px; padding: 10px 12px 12px;
    border-radius: 16px; background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline);
    transition: box-shadow 200ms; min-width: 0;
  }
  .side:focus-within { box-shadow: inset 0 0 0 1.5px var(--glow, var(--accent)); }
  .tag { font-size: 11px; color: var(--muted); letter-spacing: 0.02em; }
  .side input, .result {
    width: 100%; min-width: 0; border: 0; background: transparent; padding: 0; height: 32px;
    color: var(--fg); font: inherit; font-size: 24px; font-weight: 600; letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums; outline: none; text-align: left;
  }
  .result { cursor: copy; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--glow, var(--accent)); }
  .result span { display: inline-block; animation: rise 220ms cubic-bezier(0.2, 0.9, 0.25, 1); }
  .swap {
    width: 36px; height: 36px; display: grid; place-items: center;
    border: 0; border-radius: 50%; background: var(--surface-2); color: var(--fg); cursor: pointer;
    transition: transform 460ms cubic-bezier(0.3, 1.4, 0.4, 1), color 160ms;
  }
  .swap :global(svg) { transform: rotate(90deg); }
  .swap:hover { color: var(--glow, var(--accent)); transform: rotate(180deg); }
  .foot { display: flex; gap: 6px; min-height: 0; }
  .sample {
    flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; padding: 6px 8px;
    border: 0; border-radius: 10px; background: var(--surface-1); color: var(--muted);
    font: inherit; font-size: 11px; text-align: left; cursor: pointer; transition: background 160ms;
  }
  .sample:hover { background: var(--surface-2); }
  .sample b, .sample span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums; }
  .sample b { color: var(--fg); font-weight: 500; }
  .note { margin: -6px 0 0; font-size: 11px; color: var(--muted); }
  @keyframes rise { from { opacity: 0; transform: translateY(6px); } }
  @media (prefers-reduced-motion: reduce) {
    .result span { animation: none; }
  }
</style>
