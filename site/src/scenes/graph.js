// A pixel pup behind a knowledge graph. Edges carry pulses, picked nodes light up, and a forgetting curve
// (a synthetic illustration, not data) decays and jumps back up at each review tick.
import { registerScene } from "../kit/ascii.js";

// # body, @ ear and nose, o muzzle and paws. Eyes are holes so they can blink.
const PUP = [
  "         ############         ",
  "      ##################      ",
  "   @@@##################@@@   ",
  "  @@@@##################@@@@  ",
  "  @@@@###  ########  ###@@@@  ",
  "  @@@@###  ########  ###@@@@  ",
  "  @@@@##################@@@@  ",
  "   @@@#####ooo@@ooo#####@@@   ",
  "   @@@#####oooooooo#####@@@   ",
  "    @@#####ooo  ooo#####@@    ",
  "     @######oo  oo######@     ",
  "        #### oooo ####        ",
  "       ################       ",
  "      ##################      ",
  "     ####################     ",
  "     ####  ########  ####     ",
  "     ####  ########  ####     ",
  "     ooo#  ########  #ooo     ",
];
const EYES = [[9, 4], [10, 4], [9, 5], [10, 5], [19, 4], [20, 4], [19, 5], [20, 5]];
const TOPICS = ["recursion", "big-O", "hashing", "trees", "caching", "locks", "graphs", "parsing", "types", "queues", "joins", "retries", "sorting", "streams", "closures", "paging"];

// Illustration only: made-up intervals that grow, made-up decay. Review ticks are columns of the plot.
const PW = 30, PH = 7, BW = 40, BH = 10;
const REVIEWS = [0, 7, 16, 26];
const STAB = [12, 20, 34, 56];
const CYCLE = 11500, SCAN = 8500;

const recallAt = (x) => {
  let k = 0;
  for (let i = 0; i < REVIEWS.length; i++) if (x >= REVIEWS[i]) k = i;
  return Math.exp(-(x - REVIEWS[k]) / STAB[k]);
};
const rowAt = (r) => Math.max(0, Math.min(PH - 1, Math.round((1 - (r - 0.5) / 0.5) * (PH - 1))));

function line(x0, y0, x1, y1) {
  const out = [];
  const dx = Math.abs(x1 - x0), dy = Math.abs(y1 - y0), sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
  let err = dx - dy, x = x0, y = y0;
  while (x !== x1 || y !== y1) {
    const e2 = 2 * err;
    let mx = 0, my = 0;
    if (e2 > -dy) { err -= dy; x += sx; mx = sx; }
    if (e2 < dx) { err += dx; y += sy; my = sy; }
    const ch = my === 0 ? "-" : mx === 0 ? "|" : mx * my > 0 ? "\\" : "/";
    if (x !== x1 || y !== y1) out.push({ x, y, ch });
  }
  return out;
}

registerScene("graph", ({ cols, rows, rand }) => {
  const wide = cols >= 72;
  const x0 = wide ? Math.floor(cols * 0.57) : 1;
  const R = wide
    ? { x: x0, y: 1, w: cols - x0 - 3, h: rows - 2 }
    : { x: 1, y: Math.max(0, rows - 16), w: cols - 2, h: Math.min(rows, 16) - 1 };
  const curve = wide && R.h >= 28 && R.w >= 56;
  const box = { x: R.x + R.w - BW, y: R.y + R.h - BH };
  const pupX = R.x + Math.floor((R.w - 30) / 2) - (wide ? 4 : 0);
  const pupY = wide ? R.y + Math.max(1, Math.floor((R.h - (curve ? BH : 0) - 18) / 2)) : R.y;

  // nodes: rejection sampled, never over the curve box, never left of the headline column
  const want = Math.max(8, Math.min(22, Math.floor((R.w * R.h) / (wide ? 120 : 70))));
  const nodes = [];
  const hit = (x, y) => curve && x > box.x - 3 && y > box.y - 2;
  const dist = (a, b) => Math.hypot((a.x - b.x) * 0.5, a.y - b.y);
  for (let minD = 5; nodes.length < want && minD > 2; minD -= 0.5) {
    for (let tries = 0; tries < 300 && nodes.length < want; tries++) {
      const n = { x: R.x + 2 + Math.floor(rand() * Math.max(1, R.w - 16)), y: R.y + 1 + Math.floor(rand() * Math.max(1, R.h - 2)), lit: 0, flash: 0, label: TOPICS[nodes.length % TOPICS.length] };
      if (hit(n.x, n.y) || hit(n.x + 12, n.y)) continue;
      if (nodes.every((m) => dist(n, m) >= minD)) nodes.push(n);
    }
  }
  const edges = [], seen = new Set();
  nodes.forEach((a, i) => {
    nodes.map((b, j) => ({ j, d: dist(a, b) })).filter((o) => o.j !== i).sort((p, q) => p.d - q.d).slice(0, 2).forEach(({ j, d }, k) => {
      if (k && d > 12) return; // the second link only when it is close, so no long straight rules
      const key = i < j ? `${i}-${j}` : `${j}-${i}`;
      if (seen.has(key)) return;
      seen.add(key);
      edges.push({ a: i, b: j, cells: line(a.x, a.y, nodes[j].x, nodes[j].y), pos: -1, dir: 1, speed: 12, next: 400 + rand() * 3500 });
    });
  });
  const incident = nodes.map((_, i) => edges.filter((e) => e.a === i || e.b === i));
  const stillPicks = [Math.min(2, nodes.length - 1), Math.floor(nodes.length / 2) + 1].filter((i) => i >= 0 && i < nodes.length);

  const pulse = (e, from) => { e.dir = e.a === from ? 1 : -1; e.pos = e.dir === 1 ? 0 : e.cells.length - 1; e.speed = 10 + rand() * 8; };
  const light = (i) => { nodes[i].lit = 1; incident[i].forEach((e) => { if (e.pos < 0) pulse(e, i); }); };
  let nextPick = 900, last = -1, hovered = -1;
  const grid = new Array(cols * rows);

  return {
    step(t, dt, ptr, put) {
      const still = dt === 0;
      grid.fill(undefined);
      const set = (x, y, ch, tone = 0, a = 1, mono = false) => {
        if (x < 0 || y < 0 || x >= cols || y >= rows) return;
        grid[y * cols + x] = [ch, tone, a, mono];
      };

      // picks and pointer
      if (!still) {
        if (t >= nextPick && nodes.length) {
          let i; do { i = Math.floor(rand() * nodes.length); } while (i === last && nodes.length > 1);
          last = i; light(i); nextPick = t + 1700 + rand() * 1000;
        }
        let near = -1, best = 5;
        nodes.forEach((n, i) => { const d = Math.hypot((n.x - ptr.col) * 0.5, n.y - ptr.row); if (d < best) { best = d; near = i; } });
        if (near !== hovered && near >= 0) light(near);
        hovered = near;
        if (hovered >= 0) nodes[hovered].lit = 1;
        nodes.forEach((n) => { n.lit = Math.max(0, n.lit - dt / 3200); n.flash = Math.max(0, n.flash - dt / 500); });
      } else stillPicks.forEach((i) => { nodes[i].lit = 1; });

      // pup, dim, behind everything
      const blink = !still && t % 4300 < 150;
      PUP.forEach((row, ry) => {
        for (let rx = 0; rx < row.length; rx++) {
          const ch = row[rx];
          if (ch !== " ") set(pupX + rx, pupY + ry, ch, 0, ch === "#" ? 0.2 : 0.32);
        }
      });
      if (blink) EYES.forEach(([ex, ey]) => set(pupX + ex, pupY + ey, "#", 0, 0.2));

      // edges
      edges.forEach((e) => {
        const l = Math.max(nodes[e.a].lit, nodes[e.b].lit);
        e.cells.forEach((c) => set(c.x, c.y, c.ch, l > 0.05 ? 2 : 0, l > 0.05 ? 0.3 + 0.5 * l : 0.3));
        if (!still && e.pos < 0 && t >= e.next) pulse(e, rand() < 0.5 ? e.a : e.b);
        if (e.pos >= 0) {
          if (!still) e.pos += e.dir * e.speed * (dt / 1000);
          const i = Math.floor(e.pos);
          if (i < 0 || i >= e.cells.length) {
            nodes[e.dir === 1 ? e.b : e.a].flash = 1;
            e.pos = -1; e.next = t + 900 + rand() * 4200;
          } else {
            set(e.cells[i].x, e.cells[i].y, "*", 2, 1);
            [1, 2].forEach((k) => { const c = e.cells[i - e.dir * k]; if (c) set(c.x, c.y, k === 1 ? "+" : c.ch, 2, k === 1 ? 0.65 : 0.4); });
          }
        }
      });

      // nodes on top
      nodes.forEach((n) => {
        if (n.lit > 0.05) {
          set(n.x - 1, n.y, "(", 2, n.lit); set(n.x + 1, n.y, ")", 2, n.lit);
          set(n.x, n.y, "@", 2, 1);
          for (let k = 0; k < n.label.length; k++) set(n.x + 3 + k, n.y, n.label[k], 2, Math.min(1, n.lit * 1.4), true);
        } else if (n.flash > 0.05) set(n.x, n.y, "O", 1, 0.5 + 0.5 * n.flash);
        else set(n.x, n.y, "o", 1, 0.62);
      });

      // forgetting curve: drawn left to right by a cursor, holds, then repeats
      if (curve) {
        const px0 = box.x + 8, py0 = box.y + 1;
        const cyc = still ? CYCLE : t % CYCLE;
        const cx = still || cyc > SCAN ? PW - 1 : Math.floor((cyc / SCAN) * PW);
        const text = (x, y, s, tone, a) => { for (let i = 0; i < s.length; i++) set(x + i, y, s[i], tone, a, true); };
        text(box.x, box.y, `recall ${Math.round(recallAt(cx) * 100)}%`, 2, 0.95);
        text(box.x + BW - 22, box.y, "synthetic illustration", 0, 0.7);
        text(box.x + 2, py0, "100%", 0, 0.6);
        text(box.x + 3, py0 + PH - 1, "50%", 0, 0.6);
        for (let y = 0; y < PH; y++) set(px0 - 1, py0 + y, "|", 0, 0.45);
        set(px0 - 1, py0 + PH, "+", 0, 0.45);
        for (let x = 0; x < PW; x++) set(px0 + x, py0 + PH, "-", 0, 0.45);
        text(box.x, py0 + PH + 1, "reviews", 0, 0.6);
        REVIEWS.forEach((rx) => set(px0 + rx, py0 + PH + 1, "^", rx <= cx ? 2 : 0, rx <= cx ? 1 : 0.5));
        let prev = 0;
        for (let x = 0; x <= cx; x++) {
          const row = rowAt(recallAt(x));
          if (REVIEWS.includes(x) && x > 0) {
            for (let y = 0; y <= prev; y++) set(px0 + x, py0 + y, y === 0 ? "." : "|", 2, 0.85);
          } else set(px0 + x, py0 + row, x > 0 && row > prev ? "\\" : "_", 1, 0.85);
          prev = row;
        }
        set(px0 + cx, py0 + rowAt(recallAt(cx)), "*", 2, 1);
      }

      for (let i = 0; i < grid.length; i++) {
        const g = grid[i];
        if (g) put(i % cols, Math.floor(i / cols), g[0], g[1], g[2], g[3]);
      }
    },
  };
});
