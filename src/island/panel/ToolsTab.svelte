<script lang="ts">
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import { CATEGORIES, DEFAULT_PAIR, POPULAR_CURRENCIES, UNITS, convert, formatNumber, symbol, type Category } from "../../lib/units";
  import { rates, weather } from "../../lib/stores/weather.svelte";
  import { dayName, formatTemp, skyIcon, skyLabel, skyOf } from "../../lib/weather";

  // ---- converter

  const CONVERT_KEY = "notcher.convert";
  type Saved = { category: Category; from: string; to: string; input: string };
  function restore(): Saved {
    try {
      const s = JSON.parse(localStorage.getItem(CONVERT_KEY) ?? "null") as Saved | null;
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
      localStorage.setItem(CONVERT_KEY, JSON.stringify({ category, from, to, input }));
    } catch {
      /* not saved */
    }
  });

  $effect(() => {
    if (category === "currency") void rates.ensure();
  });

  const options = $derived.by(() => {
    if (category !== "currency") return UNITS[category].map((u) => ({ id: u.id, label: u.label }));
    const codes = Object.keys(rates.current?.rates ?? {});
    const rest = codes.filter((c) => !POPULAR_CURRENCIES.includes(c)).sort();
    return [...POPULAR_CURRENCIES.filter((c) => codes.includes(c) || !codes.length), ...rest].map((c) => ({ id: c, label: c }));
  });

  function pick(next: Category) {
    if (next === category) return;
    category = next;
    [from, to] = DEFAULT_PAIR[next];
    if (next === "temperature" && input === "1") input = "25";
  }

  const value = $derived(Number(input.replace(/[,\s]/g, "")));
  const result = $derived(input.trim() === "" ? null : convert(category, value, from, to, rates.current?.rates));
  const one = $derived(convert(category, 1, from, to, rates.current?.rates));

  function swap() {
    [from, to] = [to, from];
    if (result !== null) input = formatNumber(result).replace(/,/g, "");
  }

  let copied = $state(false);
  async function copy() {
    if (result === null) return;
    try {
      await navigator.clipboard.writeText(formatNumber(result).replace(/,/g, ""));
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

  // ---- weather

  $effect(() => weather.watch());
  const w = $derived(weather.now);
  const today = $derived(w?.days[0]?.date ?? "");
  const windText = $derived(w ? (weather.unit === "f" ? `${Math.round(w.wind / 1.609344)} mph` : `${Math.round(w.wind)} km/h`) : "");

  let editingCity = $state(false);
  let cityDraft = $state("");
  let cityInput = $state<HTMLInputElement>();
  function editCity() {
    cityDraft = weather.city;
    editingCity = true;
    queueMicrotask(() => cityInput?.focus());
  }
  function cityKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      editingCity = false;
      void weather.setCity(cityDraft);
    } else if (e.key === "Escape") {
      e.stopPropagation();
      editingCity = false;
    }
  }
</script>

<div class="tools">
  <section class="card convert">
    <div class="chips" bind:this={chipRow} onwheel={wheel} role="tablist" aria-label="What to convert">
      {#each CATEGORIES as c (c.id)}
        <button role="tab" data-id={c.id} aria-selected={category === c.id} onclick={() => pick(c.id)}>{c.label}</button>
      {/each}
    </div>

    <div class="pair">
      <label class="field">
        <input type="text" inputmode="decimal" spellcheck="false" aria-label="Amount" bind:value={input} />
        <select bind:value={from} aria-label="From">
          {#each options as o (o.id)}<option value={o.id}>{o.label}</option>{/each}
        </select>
      </label>
      <button class="swap" aria-label="Swap units" title="Swap" onclick={swap}><Icon name="swap" size={15} /></button>
      <div class="field out">
        <button class="result" title="Copy" onclick={copy} class:copied>
          {#key copied}<span>{copied ? "Copied" : formatNumber(result)}</span>{/key}
        </button>
        <select bind:value={to} aria-label="To">
          {#each options as o (o.id)}<option value={o.id}>{o.label}</option>{/each}
        </select>
      </div>
    </div>

    <p class="muted foot">
      {#if category === "currency" && !rates.current}
        {rates.error ?? "Fetching today's rates…"}
      {:else if category === "currency"}
        1 {from} = {formatNumber(one)} {to}{rates.error ? ` · ${rates.error}` : ""}
      {:else if category !== "temperature"}
        1 {symbol(from)} = {formatNumber(one)} {symbol(to)}
      {:else}
        Click the result to copy it
      {/if}
    </p>
  </section>

  <section class="card weather sky-{w ? skyOf(w.code) : 'none'}" class:night={w && !w.isDay}>
    <header>
      {#if editingCity}
        <input
          class="city"
          bind:this={cityInput}
          bind:value={cityDraft}
          onkeydown={cityKey}
          onblur={() => (editingCity = false)}
          placeholder="City (blank = automatic)"
          aria-label="City"
        />
      {:else}
        <button class="place" title="Change city" onclick={editCity}>
          <Icon name="place" size={13} />
          <span>{w?.place ?? (weather.city || "Your area")}</span>
        </button>
      {/if}
      <div class="unit" role="radiogroup" aria-label="Temperature unit">
        <button role="radio" aria-checked={weather.unit === "c"} onclick={() => weather.setUnit("c")}>°C</button>
        <button role="radio" aria-checked={weather.unit === "f"} onclick={() => weather.setUnit("f")}>°F</button>
      </div>
    </header>

    {#if w}
      <div class="now">
        <span class="big-icon"><Icon name={skyIcon(w.code, w.isDay) as IconName} size={40} /></span>
        <div class="reading">
          <span class="temp">{formatTemp(w.temp, weather.unit)}</span>
          <span class="cond">{skyLabel(w.code)}</span>
        </div>
        <dl>
          <div><dt>Feels</dt><dd>{formatTemp(w.feels, weather.unit)}</dd></div>
          <div><dt>Humidity</dt><dd>{Math.round(w.humidity)}%</dd></div>
          <div><dt>Wind</dt><dd>{windText}</dd></div>
        </dl>
      </div>
      <ol class="days">
        {#each w.days as d (d.date)}
          <li>
            <span class="muted">{dayName(d.date, today)}</span>
            <Icon name={skyIcon(d.code) as IconName} size={17} />
            <span class="range"><b>{formatTemp(d.max, weather.unit)}</b> {formatTemp(d.min, weather.unit)}</span>
          </li>
        {/each}
      </ol>
    {:else if weather.error}
      <div class="empty">
        <span>{weather.error}</span>
        <button class="retry" onclick={() => weather.refresh(true)}><Icon name="refresh" size={14} />Try again</button>
      </div>
    {:else}
      <div class="empty"><span class="muted">Checking the sky…</span></div>
    {/if}
  </section>
</div>

<style>
  .tools { height: 100%; display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .card {
    position: relative;
    display: flex; flex-direction: column; gap: 10px;
    padding: 12px 14px; border-radius: 16px; min-width: 0; overflow: hidden;
    background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline);
  }
  .muted { color: var(--muted); }

  /* converter */
  .chips {
    display: flex; gap: 4px; overflow-x: auto; scrollbar-width: none; flex: none;
    mask-image: linear-gradient(90deg, #000 85%, transparent);
    margin: 0 -2px; padding: 0 2px;
  }
  .chips::-webkit-scrollbar { display: none; }
  .chips button {
    flex: none; border: 0; padding: 5px 11px; border-radius: 999px;
    background: transparent; color: var(--muted); font: inherit; font-size: 12px; cursor: pointer;
    transition: background 200ms, color 200ms;
  }
  .chips button:hover { color: var(--fg); }
  .chips button[aria-selected="true"] { background: var(--surface-2); color: var(--fg); }
  .pair { position: relative; display: flex; flex-direction: column; gap: 6px; flex: 1; justify-content: center; }
  .field {
    display: flex; align-items: center; gap: 8px; height: 50px; padding: 0 6px 0 14px;
    border-radius: 13px; background: var(--surface-2);
    transition: box-shadow 200ms;
  }
  .field:focus-within { box-shadow: inset 0 0 0 1.5px var(--glow, var(--accent)); }
  .field input, .result {
    flex: 1; min-width: 0; border: 0; background: transparent; padding: 0;
    color: var(--fg); font: inherit; font-size: 20px; font-weight: 600; letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums; outline: none; text-align: left;
  }
  .result { cursor: copy; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--glow, var(--accent)); }
  .result span { display: inline-block; animation: rise 220ms cubic-bezier(0.2, 0.9, 0.25, 1); }
  .result.copied { color: var(--fg); font-size: 15px; }
  .field select {
    flex: none; max-width: 46%; font: inherit; font-size: 12px; color: var(--fg);
    background: var(--surface-1); border: 0; border-radius: 9px; padding: 6px 6px; cursor: pointer;
  }
  .swap {
    position: absolute; right: 50%; top: 50%; translate: 50% -50%; z-index: 1;
    width: 28px; height: 28px; display: grid; place-items: center;
    border: 3px solid var(--island-bg, #000); border-radius: 50%;
    background: var(--surface-2); color: var(--fg); cursor: pointer;
    transition: transform 420ms cubic-bezier(0.3, 1.4, 0.4, 1), color 160ms;
  }
  .swap:hover { color: var(--glow, var(--accent)); transform: rotate(180deg); }
  .foot { margin: 0; font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* weather */
  .weather::before {
    content: ""; position: absolute; inset: -40% -20% auto auto; width: 75%; aspect-ratio: 1;
    border-radius: 50%; filter: blur(34px); opacity: 0.32; pointer-events: none;
    background: var(--sky, transparent); transition: background 600ms;
  }
  .sky-clear { --sky: #ffb547; }
  .sky-partly { --sky: #ffc56e; }
  .sky-cloudy, .sky-fog { --sky: #9aa7c7; }
  .sky-drizzle, .sky-rain { --sky: #4f8dff; }
  .sky-snow { --sky: #cfe3ff; }
  .sky-storm { --sky: #8b5cff; }
  .night { --sky: #6c63ff; }
  .weather > * { position: relative; }
  header { display: flex; align-items: center; gap: 8px; height: 26px; }
  .place {
    display: inline-flex; align-items: center; gap: 5px; min-width: 0;
    border: 0; padding: 3px 6px 3px 2px; border-radius: 8px; background: transparent;
    color: var(--fg); font: inherit; font-size: 12.5px; font-weight: 600; cursor: pointer;
  }
  .place span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .place:hover { background: var(--surface-2); }
  .place :global(svg) { color: var(--glow, var(--accent)); flex: none; }
  .city {
    flex: 1; min-width: 0; height: 26px; padding: 0 8px; border: 0; border-radius: 8px; outline: none;
    background: var(--surface-2); color: var(--fg); font: inherit; font-size: 12.5px;
    box-shadow: inset 0 0 0 1.5px var(--glow, var(--accent));
  }
  .unit { margin-left: auto; display: inline-flex; padding: 2px; border-radius: 8px; background: var(--surface-2); flex: none; }
  .unit button {
    border: 0; padding: 2px 7px; border-radius: 6px; background: transparent;
    color: var(--muted); font: inherit; font-size: 11px; cursor: pointer; transition: background 180ms, color 180ms;
  }
  .unit button[aria-checked="true"] { background: var(--surface-1); color: var(--fg); }
  .now { display: flex; align-items: center; gap: 10px; flex: 1; min-height: 0; }
  .big-icon { color: var(--glow, var(--accent)); display: grid; animation: drift 6s ease-in-out infinite; }
  .reading { display: flex; flex-direction: column; min-width: 0; }
  .temp { font-size: 34px; font-weight: 600; line-height: 1; letter-spacing: -0.03em; font-variant-numeric: tabular-nums; }
  .cond { font-size: 12px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  dl { margin: 0 0 0 auto; display: flex; flex-direction: column; gap: 2px; font-size: 11px; flex: none; }
  dl div { display: flex; justify-content: space-between; gap: 8px; }
  dt { color: var(--muted); }
  dd { margin: 0; font-variant-numeric: tabular-nums; }
  .days { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(5, 1fr); gap: 4px; }
  .days li {
    display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 7px 0;
    border-radius: 11px; background: var(--surface-2); font-size: 11px;
  }
  .days li:first-child { box-shadow: inset 0 0 0 1px var(--hairline); }
  .days :global(svg) { color: var(--fg); opacity: 0.85; }
  .range { color: var(--muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .range b { color: var(--fg); font-weight: 600; }
  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; font-size: 12.5px; text-align: center; }
  .retry {
    display: inline-flex; align-items: center; gap: 6px; border: 0; padding: 6px 12px; border-radius: 999px;
    background: var(--surface-2); color: var(--fg); font: inherit; font-size: 12px; cursor: pointer;
  }
  @keyframes rise { from { opacity: 0; transform: translateY(6px); } }
  @keyframes drift { 50% { transform: translateY(-3px); } }
  @media (prefers-reduced-motion: reduce) {
    .big-icon, .result span { animation: none; }
  }
</style>
