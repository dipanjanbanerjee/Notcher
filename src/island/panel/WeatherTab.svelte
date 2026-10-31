<script lang="ts">
  import { tick } from "svelte";
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import { weather } from "../../lib/stores/weather.svelte";
  import { dayName, formatHm, formatTemp, hourName, skyIcon, skyLabel, skyOf, type Place } from "../../lib/weather";

  $effect(() => weather.watch());
  const w = $derived(weather.now);
  const today = $derived(w?.days[0]?.date ?? "");
  const unit = $derived(weather.unit);
  const wind = $derived(w ? (unit === "f" ? `${Math.round(w.wind / 1.609344)} mph` : `${Math.round(w.wind)} km/h`) : "");
  const icon = (code: number, isDay = true) => skyIcon(code, isDay) as IconName;

  // ---- choosing a place

  let searching = $state(false);
  let query = $state("");
  let results = $state<Place[]>([]);
  let busy = $state(false);
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let seq = 0;

  async function openSearch() {
    searching = true;
    query = "";
    results = [];
    active = 0;
    await tick();
    input?.focus();
  }

  $effect(() => {
    const q = query.trim();
    const mine = ++seq;
    if (q.length < 2) {
      results = [];
      busy = false;
      return;
    }
    busy = true;
    const t = setTimeout(async () => {
      const found = await weather.search(q).catch(() => []);
      if (mine !== seq) return;
      results = found;
      busy = false;
    }, 350);
    return () => clearTimeout(t);
  });

  function choose(place: Place | null) {
    searching = false;
    void weather.choose(place);
  }

  function onKey(e: KeyboardEvent) {
    // The list is "Use my location" followed by the results.
    const count = results.length + 1;
    if (e.key === "Escape") {
      e.stopPropagation();
      searching = false;
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = (active + (e.key === "ArrowDown" ? 1 : count - 1)) % count;
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (active === 0) choose(null);
      else if (results[active - 1]) choose(results[active - 1]);
    }
  }
  // With matches listed, Enter picks the first match rather than "Use my location".
  $effect(() => {
    active = results.length ? 1 : 0;
  });

  let hourRow = $state<HTMLOListElement>();
  function wheel(e: WheelEvent) {
    if (hourRow && Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      hourRow.scrollBy({ left: e.deltaY, behavior: "smooth" });
      e.preventDefault();
    }
  }
</script>

<div class="weather sky-{w ? skyOf(w.code) : 'none'}" class:night={w && !w.isDay}>
  <header>
    <button class="place" title="Change place" onclick={openSearch}>
      <Icon name="place" size={14} />
      <span class="name">{w?.place ?? weather.picked?.name ?? "Your location"}</span>
      {#if w?.detail}<span class="detail">{w.detail}</span>{/if}
      <svg class="chev" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 10l5 5 5-5" /></svg>
    </button>
    {#if w?.approximate}
      <span class="approx" title="Guessed from your internet connection. Turn on Settings › Privacy & security › Location › Let desktop apps access your location in Windows for your exact area, or pick a place.">Approximate</span>
    {/if}
    <div class="right">
      <div class="unit" role="radiogroup" aria-label="Temperature unit">
        <button role="radio" aria-checked={unit === "c"} onclick={() => weather.setUnit("c")}>°C</button>
        <button role="radio" aria-checked={unit === "f"} onclick={() => weather.setUnit("f")}>°F</button>
      </div>
      <button class="icon-btn" class:spin={weather.loading} aria-label="Refresh" title="Refresh" onclick={() => weather.refresh(true)}>
        <Icon name="refresh" size={14} />
      </button>
    </div>
  </header>

  {#if w}
    <div class="grid">
      <section class="card now">
        <div class="hero">
          <span class="big-icon"><Icon name={icon(w.code, w.isDay)} size={44} /></span>
          <div class="reading">
            <span class="temp">{formatTemp(w.temp, unit)}</span>
            <span class="cond">{skyLabel(w.code)}</span>
          </div>
        </div>
        {#if w.days[0]}
          <p class="range">High {formatTemp(w.days[0].max, unit)} · Low {formatTemp(w.days[0].min, unit)}</p>
        {/if}
        <dl>
          <div><dt>Feels like</dt><dd>{formatTemp(w.feels, unit)}</dd></div>
          <div><dt>Humidity</dt><dd>{Math.round(w.humidity)}%</dd></div>
          <div><dt>Wind</dt><dd>{wind}</dd></div>
          <div><dt>Rain</dt><dd>{w.hours[0]?.rain ?? w.days[0]?.rain ?? 0}%</dd></div>
          <div><dt>Sunrise</dt><dd>{formatHm(w.sunrise, weather.clock24h)}</dd></div>
          <div><dt>Sunset</dt><dd>{formatHm(w.sunset, weather.clock24h)}</dd></div>
        </dl>
      </section>

      <div class="side">
        <section class="card hours">
          <ol bind:this={hourRow} onwheel={wheel} aria-label="Next hours">
            {#each w.hours.slice(0, 24) as h, i (h.time)}
              <li class:first={i === 0}>
                <span class="muted">{hourName(h.time, i === 0, weather.clock24h)}</span>
                <Icon name={icon(h.code, h.isDay)} size={18} />
                <b>{formatTemp(h.temp, unit)}</b>
                <span class="rain" class:dry={h.rain < 20}>{h.rain}%</span>
              </li>
            {/each}
          </ol>
        </section>
        <section class="card days">
          <ol aria-label="This week">
            {#each w.days as d (d.date)}
              <li>
                <span class="muted">{dayName(d.date, today)}</span>
                <Icon name={icon(d.code)} size={17} />
                <span class="mm"><b>{formatTemp(d.max, unit)}</b> {formatTemp(d.min, unit)}</span>
              </li>
            {/each}
          </ol>
        </section>
      </div>
    </div>
  {:else if weather.error}
    <div class="empty">
      <span>{weather.error}</span>
      <div class="row">
        <button class="pill" onclick={() => weather.refresh(true)}><Icon name="refresh" size={14} />Try again</button>
        <button class="pill" onclick={openSearch}><Icon name="place" size={14} />Pick a place</button>
      </div>
    </div>
  {:else}
    <div class="empty"><span class="muted">Checking the sky…</span></div>
  {/if}

  {#if searching}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="search" onkeydown={onKey}>
      <div class="field">
        <Icon name="search" size={15} />
        <input
          bind:this={input}
          bind:value={query}
          placeholder="City, area or PIN code"
          aria-label="Search for a place"
          spellcheck="false"
          autocomplete="off"
        />
        <button class="icon-btn" aria-label="Close" onclick={() => (searching = false)}><Icon name="close" size={14} /></button>
      </div>
      <ul role="listbox" aria-label="Places">
        <!-- svelte-ignore a11y_click_events_have_key_events (keys are handled on the list) -->
        <li role="option" aria-selected={active === 0} class:active={active === 0} onpointerenter={() => (active = 0)} onclick={() => choose(null)}>
          <Icon name="place" size={14} />
          <span class="name">Use my location</span>
          <span class="detail">Automatic</span>
        </li>
        {#each results as r, i (r.lat + "," + r.lon)}
          <!-- svelte-ignore a11y_click_events_have_key_events (keys are handled on the list) -->
          <li role="option" aria-selected={active === i + 1} class:active={active === i + 1} onpointerenter={() => (active = i + 1)} onclick={() => choose(r)}>
            <span class="dot"></span>
            <span class="name">{r.name}</span>
            <span class="detail">{r.detail}</span>
          </li>
        {/each}
        {#if busy}
          <li class="hint">Searching…</li>
        {:else if query.trim().length >= 2 && !results.length}
          <li class="hint">No places found. Try a nearby city or a PIN code</li>
        {/if}
      </ul>
    </div>
  {/if}
</div>

<style>
  .weather { position: relative; height: 100%; display: flex; flex-direction: column; gap: 10px; overflow: hidden; border-radius: 16px; }
  .weather::before {
    content: ""; position: absolute; top: -30px; left: -30px; width: 300px; height: 240px;
    border-radius: 50%; filter: blur(50px); opacity: 0.22; pointer-events: none;
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
  .muted { color: var(--muted); }

  header { display: flex; align-items: center; gap: 8px; height: 28px; flex: none; }
  .place {
    display: inline-flex; align-items: baseline; gap: 6px; min-width: 0; max-width: 70%;
    border: 0; padding: 5px 8px; border-radius: 9px; background: transparent;
    color: var(--fg); font: inherit; cursor: pointer; transition: background 160ms;
  }
  .place:hover { background: var(--surface-2); }
  .place :global(svg) { color: var(--glow, var(--accent)); flex: none; align-self: center; }
  .place .name { font-size: 13.5px; font-weight: 600; white-space: nowrap; }
  .place .detail { font-size: 11.5px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .chev { width: 14px; height: 14px; flex: none; align-self: center; fill: none; stroke: var(--muted); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .approx {
    font-size: 10.5px; padding: 2px 7px; border-radius: 999px; cursor: help; white-space: nowrap;
    color: #ffcf8a; background: rgb(255 180 80 / 0.12); box-shadow: inset 0 0 0 1px rgb(255 180 80 / 0.25);
  }
  .right { margin-left: auto; display: flex; align-items: center; gap: 6px; }
  .unit { display: inline-flex; padding: 2px; border-radius: 8px; background: var(--surface-1); }
  .unit button {
    border: 0; padding: 3px 8px; border-radius: 6px; background: transparent;
    color: var(--muted); font: inherit; font-size: 11px; cursor: pointer; transition: background 180ms, color 180ms;
  }
  .unit button[aria-checked="true"] { background: var(--surface-2); color: var(--fg); }
  .icon-btn {
    width: 26px; height: 26px; display: grid; place-items: center; flex: none;
    border: 0; border-radius: 8px; background: transparent; color: var(--muted); cursor: pointer;
  }
  .icon-btn:hover { background: var(--surface-2); color: var(--fg); }
  .spin :global(svg) { animation: spin 900ms linear infinite; }

  .grid { flex: 1; min-height: 0; display: grid; grid-template-columns: 218px 1fr; gap: 10px; }
  .card { border-radius: 16px; background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline); min-width: 0; overflow: hidden; }
  .now { display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; justify-content: space-between; }
  .hero { display: flex; align-items: center; gap: 10px; }
  .big-icon { color: var(--glow, var(--accent)); display: grid; }
  .reading { display: flex; flex-direction: column; min-width: 0; }
  .temp { font-size: 36px; font-weight: 600; line-height: 1; letter-spacing: -0.03em; font-variant-numeric: tabular-nums; }
  .cond { font-size: 12px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .range { margin: 0; font-size: 11.5px; color: var(--muted); }
  dl { margin: 0; display: grid; grid-template-columns: 1fr 1fr; gap: 4px 12px; font-size: 11px; }
  dl div { display: flex; justify-content: space-between; gap: 6px; }
  dt { color: var(--muted); }
  dd { margin: 0; font-variant-numeric: tabular-nums; white-space: nowrap; }

  .side { display: flex; flex-direction: column; gap: 10px; min-width: 0; min-height: 0; }
  ol { list-style: none; margin: 0; padding: 0; }
  .hours { flex: 1; min-height: 0; }
  .hours ol {
    height: 100%; display: flex; gap: 2px; padding: 6px; overflow-x: auto; scrollbar-width: none;
    mask-image: linear-gradient(90deg, #000 88%, transparent);
  }
  .hours ol::-webkit-scrollbar { display: none; }
  .hours li {
    flex: 0 0 46px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 3px;
    border-radius: 11px; font-size: 11px;
  }
  .hours li.first { background: var(--surface-2); }
  .hours li b { font-weight: 600; font-variant-numeric: tabular-nums; }
  .rain { font-size: 10px; color: #7fb2ff; font-variant-numeric: tabular-nums; }
  .rain.dry { visibility: hidden; }
  .days ol { display: grid; grid-template-columns: repeat(7, 1fr); padding: 6px; gap: 2px; }
  .days li { display: flex; flex-direction: column; align-items: center; gap: 3px; padding: 5px 0; font-size: 11px; border-radius: 10px; }
  .days li:first-child { background: var(--surface-2); }
  .mm { color: var(--muted); white-space: nowrap; font-variant-numeric: tabular-nums; font-size: 10.5px; }
  .mm b { color: var(--fg); font-weight: 600; }

  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; font-size: 12.5px; text-align: center; }
  .row { display: flex; gap: 8px; }
  .pill {
    display: inline-flex; align-items: center; gap: 6px; border: 0; padding: 6px 12px; border-radius: 999px;
    background: var(--surface-2); color: var(--fg); font: inherit; font-size: 12px; cursor: pointer;
  }

  .search {
    position: absolute; z-index: 5; top: 0; left: 0; right: 0; max-height: 100%;
    display: flex; flex-direction: column; gap: 6px; padding: 8px;
    border-radius: 16px; background: var(--island-bg);
    box-shadow: 0 0 0 1px var(--hairline), 0 18px 40px var(--shadow);
    animation: drop 200ms cubic-bezier(0.2, 0.9, 0.25, 1);
  }
  .field {
    display: flex; align-items: center; gap: 8px; height: 38px; padding: 0 6px 0 12px;
    border-radius: 11px; background: var(--surface-2); color: var(--muted);
    box-shadow: inset 0 0 0 1.5px var(--glow, var(--accent));
  }
  .field input { flex: 1; min-width: 0; border: 0; outline: none; background: transparent; color: var(--fg); font: inherit; font-size: 13.5px; }
  .search ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; max-height: 180px; }
  .search li {
    display: flex; align-items: baseline; gap: 8px; padding: 8px 10px; border-radius: 9px; cursor: pointer; font-size: 12.5px;
  }
  .search li :global(svg) { color: var(--glow, var(--accent)); align-self: center; }
  .search li.active { background: var(--surface-2); }
  .search .name { font-weight: 600; white-space: nowrap; }
  .search .detail { color: var(--muted); font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .search .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--muted); flex: none; align-self: center; margin: 0 4px; }
  .search .hint { color: var(--muted); cursor: default; }
  @keyframes drop { from { opacity: 0; transform: translateY(-6px); } }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
