// Small, dependency-free motion for the Notcher site.
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;

/** A damped spring towards `target`, like the ones the app uses. */
function spring(initial, { stiffness = 380, damping = 30 } = {}) {
  // Same numbers as the island's shape spring in the app.
  const s = { value: { ...initial }, velocity: {}, target: { ...initial } };
  for (const k in initial) s.velocity[k] = 0;
  s.step = (dt) => {
    let moving = false;
    for (const k in s.target) {
      const x = s.value[k];
      const v = s.velocity[k];
      const a = stiffness * (s.target[k] - x) - damping * v;
      s.velocity[k] = v + a * dt;
      s.value[k] = x + s.velocity[k] * dt;
      if (Math.abs(s.target[k] - s.value[k]) > 0.05 || Math.abs(s.velocity[k]) > 0.05) moving = true;
      else {
        s.value[k] = s.target[k];
        s.velocity[k] = 0;
      }
    }
    return moving;
  };
  return s;
}

/** Runs `springs` each frame while any of them is moving, calling `draw` after each step. */
function animate(s, draw) {
  if (reduced) {
    s.value = { ...s.target };
    draw(s.value);
    return;
  }
  if (s.running) return;
  s.running = true;
  let last = performance.now();
  const frame = (now) => {
    const dt = Math.min(0.032, (now - last) / 1000);
    last = now;
    // Two half steps keep the spring stable on slow frames.
    let moving = s.step(dt / 2);
    moving = s.step(dt / 2) || moving;
    draw(s.value);
    if (moving) requestAnimationFrame(frame);
    else s.running = false;
  };
  requestAnimationFrame(frame);
}

// ---- the island at the top of the page

const island = document.getElementById("island");
const scenes = [...island.querySelectorAll(".scene")];
const SIZES = {
  rest: { w: 190, h: 34, r: 17 },
  music: { w: 320, h: 54, r: 24 },
  weather: { w: 290, h: 54, r: 24 },
  water: { w: 296, h: 54, r: 24 },
  copy: { w: 270, h: 54, r: 24 },
};
const islandSpring = spring(SIZES.rest);
const drawIsland = (v) => {
  island.style.width = `${v.w}px`;
  island.style.height = `${v.h}px`;
  island.style.borderRadius = `0 0 ${v.r}px ${v.r}px`;
};
function show(name) {
  scenes.forEach((s) => s.classList.toggle("on", s.dataset.scene === name));
  islandSpring.target = { ...SIZES[name] };
  animate(islandSpring, drawIsland);
}

const clock = document.getElementById("clock");
const date = island.querySelector(".rest-date");
function tickClock() {
  const now = new Date();
  clock.textContent = now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  date.textContent = now.toLocaleDateString([], { weekday: "short", day: "numeric" });
}
tickClock();
setInterval(tickClock, 10_000);

const SEQUENCE = ["rest", "music", "rest", "weather", "rest", "copy", "rest", "water"];
let step = 0;
show("rest");
function next() {
  step = (step + 1) % SEQUENCE.length;
  show(SEQUENCE[step]);
  setTimeout(next, SEQUENCE[step] === "rest" ? 2000 : 3200);
}
if (!reduced) setTimeout(next, 1400);

// ---- screenshot tabs

const tabs = [...document.querySelectorAll(".tabs [data-shot]")];
const shots = [...document.querySelectorAll(".shots img")];
let auto = true;
function pick(name) {
  tabs.forEach((t) => t.setAttribute("aria-selected", String(t.dataset.shot === name)));
  shots.forEach((s) => s.classList.toggle("on", s.dataset.shot === name));
}
tabs.forEach((t) =>
  t.addEventListener("click", () => {
    auto = false;
    pick(t.dataset.shot);
  }),
);
let showcaseVisible = false;
new IntersectionObserver(([e]) => (showcaseVisible = e.isIntersecting)).observe(document.querySelector(".stage"));
setInterval(() => {
  if (!auto || !showcaseVisible || reduced) return;
  const i = tabs.findIndex((t) => t.getAttribute("aria-selected") === "true");
  pick(tabs[(i + 1) % tabs.length].dataset.shot);
}, 4200);

// ---- the spring demo

const pill = document.getElementById("pill");
const curve = document.getElementById("curve");
const box = document.querySelector(".spring");
const PILL = { closed: { w: 150, h: 30, r: 15 }, open: { w: 270, h: 96, r: 30 } };
const pillSpring = spring(PILL.closed);
const drawPill = (v) => {
  pill.style.width = `${v.w}px`;
  pill.style.height = `${v.h}px`;
  pill.style.borderRadius = `0 0 ${v.r}px ${v.r}px`;
};

/** Plots how the spring moves from 0 to 1, overshoot and all. */
function plot() {
  const s = spring({ x: 0 });
  s.target = { x: 1 };
  const pts = [];
  for (let i = 0; i <= 90; i++) {
    pts.push(`${(i / 90) * 300},${110 - s.value.x * 80}`);
    s.step(0.55 / 180);
    s.step(0.55 / 180);
  }
  curve.setAttribute("d", `M ${pts.join(" L ")}`);
  const len = curve.getTotalLength();
  curve.style.strokeDasharray = `${len}`;
  curve.style.strokeDashoffset = `${len}`;
  curve.getBoundingClientRect();
  curve.style.transition = "stroke-dashoffset 900ms cubic-bezier(0.2, 0.9, 0.25, 1)";
  curve.style.strokeDashoffset = "0";
}

let open = false;
function setPill(next) {
  if (next === open) return;
  open = next;
  pillSpring.target = { ...(open ? PILL.open : PILL.closed) };
  animate(pillSpring, drawPill);
  box.classList.toggle("open", open);
  if (open) plot();
}
box.addEventListener("pointerenter", () => setPill(true));
box.addEventListener("pointerleave", () => setPill(false));
box.addEventListener("click", () => setPill(!open));
let springVisible = false;
let plotted = false;
new IntersectionObserver(([e]) => {
  springVisible = e.isIntersecting;
  if (springVisible && !plotted) {
    plotted = true;
    plot();
  }
}).observe(box);
setInterval(() => {
  if (springVisible && !box.matches(":hover") && !reduced) setPill(!open);
}, 2600);

// ---- reveal sections as they scroll in

const io = new IntersectionObserver(
  (entries) =>
    entries.forEach((e) => {
      if (e.isIntersecting) {
        e.target.classList.add("seen");
        io.unobserve(e.target);
      }
    }),
  { threshold: 0.15 },
);
document.querySelectorAll(".showcase, .features h2, .grid article, .feel > *, .download > *, .steps li").forEach((el, i) => {
  el.classList.add("reveal");
  el.style.transitionDelay = `${(i % 3) * 70}ms`;
  io.observe(el);
});

// ---- point the download buttons at the newest installer

fetch("https://api.github.com/repos/dipanjanbanerjee/notcher/releases/latest")
  .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
  .then((release) => {
    const exe =
      release.assets.find((a) => a.name === "Notcher-Setup.exe") ??
      release.assets.find((a) => /setup\.exe$/i.test(a.name)) ??
      release.assets.find((a) => /\.exe$/i.test(a.name));
    if (exe) document.querySelectorAll("[data-download]").forEach((a) => (a.href = exe.browser_download_url));
    const version = String(release.tag_name).replace(/^v/, "");
    document.querySelectorAll("[data-version]").forEach((el) => (el.textContent = version));
    const when = new Date(release.published_at).toLocaleDateString([], { day: "numeric", month: "long", year: "numeric" });
    const note = document.querySelector("[data-release-note]");
    if (note) note.textContent = `Version ${version}, released ${when}`;
  })
  .catch(() => {
    /* the links already go to the releases page */
  });
