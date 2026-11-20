/** Copied from local/参考项目/focus_switch.html — focus-switch custom element. */
(() => {
'use strict';
const QS = new URLSearchParams(location.search);
const FORCE_NOGL = QS.has('nogl');
const FORCE_RM = QS.has('rm');

/* =====================================================================
   一、调色板：5 基础色 × 4 档 OKLab 明度（超色域逐步降饱和）= 20 色
   ===================================================================== */
const s2l = c => (c /= 255) <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
const l2s = c => c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
function lin2lab(r, g, b) {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
          1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
          0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s];
}
function lab2lin(L, a, b) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3;
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
         -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
         -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s];
}
const hexRGB = h => [1, 3, 5].map(i => parseInt(h.substr(i, 2), 16));
const BASES = [
  ['#f5ecd9', [.76, .85, .93, .985]],   // bg
  ['#8c9a62', [.40, .52, .64, .76]],    // surface
  ['#1b2146', [.17, .25, .33, .44]],    // fg
  ['#7f97b3', [.50, .62, .74, .86]],    // muted
  ['#efa02c', [.50, .66, .80, .92]],    // accent
];
const PAL = new Uint8Array(20 * 3);
const PALF = [];
BASES.forEach(([hex, Ls], f) => {
  const [L0, a0, b0] = lin2lab(...hexRGB(hex).map(s2l));
  const C0 = Math.hypot(a0, b0), hue = Math.atan2(b0, a0);
  Ls.forEach((L, t) => {
    let C = C0, rgb;
    for (let i = 0; i < 400; i++) {
      rgb = lab2lin(L, C * Math.cos(hue), C * Math.sin(hue));
      if (rgb.every(v => v >= -1e-5 && v <= 1 + 1e-5)) break;
      C *= 0.97;
    }
    const sr = rgb.map(v => Math.max(0, Math.min(1, l2s(Math.max(0, v)))));
    const idx = f * 4 + t;
    sr.forEach((v, j) => PAL[idx * 3 + j] = Math.round(v * 255));
    PALF[idx] = sr;
  });
});
const hexOf = i => '#' + [0, 1, 2].map(j => PAL[i * 3 + j].toString(16).padStart(2, '0')).join('');
// 编号：bg 0-3, surface 4-7, fg 8-11, muted 12-15, accent 16-19
const B0 = 0, B1 = 1, B2 = 2, B3 = 3, S0 = 4, S1 = 5, S2 = 6, S3 = 7, F0 = 8, F1 = 9, F2 = 10, F3 = 11,
      M0 = 12, M1 = 13, M2 = 14, M3 = 15, A0 = 16, A1 = 17, A2 = 18, A3 = 19;
document.documentElement.style.setProperty('--navy', hexOf(F0));
const famOf = i => i >> 2, toneOf = i => i & 3;
const DK = i => toneOf(i) > 0 ? i - 1 : (famOf(i) === 2 ? i : F3);      // 阴影：暗一档
const LT = i => toneOf(i) < 3 ? i + 1 : (famOf(i) === 0 ? i : B3);      // 日光：亮一档

/* =====================================================================
   材质：每种材质存 4 列（白天 / 黄昏 / 傍晚 / 夜）
   编号 0-19 = 恒定颜色（窗外景色、屏幕等发光体），20+ = 室内材质
   ===================================================================== */
const MT = new Uint8Array(256 * 4), LAMPI = new Uint8Array(256), DKT = new Uint8Array(256), LTT = new Uint8Array(256);
const WALLISH = new Uint8Array(256);
let nMat = 20;
for (let i = 0; i < 20; i++) { for (let c = 0; c < 4; c++) MT[i * 4 + c] = i; LAMPI[i] = i; DKT[i] = DK(i); LTT[i] = LT(i); }
function MAT(cols, wall) {
  const id = nMat++;
  for (let c = 0; c < 4; c++) MT[id * 4 + c] = cols[c];
  WALLISH[id] = wall ? 1 : 0;
  LAMPI[id] = 16 + Math.min(3, toneOf(cols[0]) + (wall ? 1 : 0));   // 灯光：换成琥珀色（墙上亮一档）
  return id;
}
const der = d => [d, DK(d), DK(DK(d)), DK(DK(DK(d)))];
const WALL = MAT([B2, B1, B0, F3], 1);
const CEIL = MAT([B1, B0, F3, F2], 1);
const WHITE = MAT([B3, B2, M1, F3]);
const TRIM = MAT([B1, B0, M0, F2], 1);
const PANEL = MAT([B3, B2, M1, F3], 1);
const BASE = MAT([B2, B1, M0, F2]);
const FLOOR = MAT([A1, A0, F3, F1]);
const SEAM = MAT([A0, F3, F2, F0]);
const DTOP = MAT([A2, A1, A0, F2]);
const DFRONT = MAT([A1, A0, F3, F1]);
const DSEAM = MAT([A0, F3, F2, F0]);
const DARK = MAT([F1, F1, F0, F0]);
const DARK2 = MAT([F2, F2, F1, F0]);
const HAIR = MAT([F0, F0, F0, F0]);
const HAIRHI = MAT([F2, F2, F1, F1]);
const SKIN = MAT([A2, A1, A0, F3]);
const SKIND = MAT([A1, A0, F3, F2]);
const SHIRT = MAT([S2, S1, S0, F1]);
const SHIRTD = MAT([S1, S0, F2, F0]);
const PANTS = MAT([F2, F1, F1, F0]);
const PANTSD = MAT([F1, F0, F0, F0]);
const LEAF = MAT([S1, S0, F2, F1]);
const LEAFL = MAT([S2, S1, S0, F2]);
const POT = MAT([A1, A0, F3, F1]);
const RUG = MAT([M1, M0, F3, F1]);
const RUGB = MAT([M0, F3, F2, F0]);
const RUGL = MAT([M2, M1, M0, F2]);
const BLIND = MAT([B2, B1, M0, F3]);
const BLINDD = MAT([B1, B0, F3, F2]);
const BK = [MAT(der(A1)), MAT([M1, M0, F3, F2]), MAT(der(S2)), MAT([F2, F1, F0, F0]), MAT(der(B1)), MAT(der(M2)), MAT(der(A0)), MAT(der(S1))];
const LAMPB = MAT([F2, F1, F1, F0]);
const SHADE = MAT([A1, A0, F3, F1]);
const KETTLE = MAT([M2, M1, M0, F2]);
const KETTLED = MAT([M1, M0, F3, F1]);
const BEANS = MAT([A0, F3, F2, F0]);
const JAR = MAT([B3, B2, M1, F3]);
const SBACK = MAT([A0, F3, F2, F0]);
const BASKET = MAT([A2, A1, A0, F3]);
const BASKETD = MAT([A1, A0, F3, F2]);
const STEAM = MAT([B3, B3, M2, M1]);

/* =====================================================================
   像素世界缓冲：核心 192×112，四周 14 像素余量 → 220×140
   ===================================================================== */
const OFF = 14, WW = 192 + OFF * 2, WH = 112 + OFF * 2, N = WW * WH;
const mat = new Uint8Array(N), vari = new Uint8Array(N), emis = new Uint8Array(N);
const RGBA = new Uint8Array(N * 4);
const BAY = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map(v => (v + 0.5) / 16);
const bay = (x, y) => BAY[(((y + OFF) & 3) << 2) | ((x + OFF) & 3)];
function P(x, y, m, v = 0, e = 0) {
  x = Math.round(x); y = Math.round(y);
  const X = x + OFF, Y = y + OFF;
  if (X < 0 || Y < 0 || X >= WW || Y >= WH) return;
  const i = Y * WW + X; mat[i] = m; vari[i] = v; emis[i] = e;
}
function R(x0, y0, x1, y1, m, v = 0, e = 0) { for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) P(x, y, m, v, e); }
function VAR(x, y, v) { const X = x + OFF, Y = y + OFF; if (X < 0 || Y < 0 || X >= WW || Y >= WH) return; vari[Y * WW + X] = v; }
function getM(x, y) { const X = x + OFF, Y = y + OFF; if (X < 0 || Y < 0 || X >= WW || Y >= WH) return -1; return mat[Y * WW + X]; }
function line(x0, y0, x1, y1, m, v = 0) {
  x0 = Math.round(x0); y0 = Math.round(y0); x1 = Math.round(x1); y1 = Math.round(y1);
  const dx = Math.abs(x1 - x0), dy = -Math.abs(y1 - y0), sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  for (let n = 0; n < 400; n++) { P(x0, y0, m, v); if (x0 === x1 && y0 === y1) break; const e2 = 2 * err; if (e2 >= dy) { err += dy; x0 += sx; } if (e2 <= dx) { err += dx; y0 += sy; } }
}
const clamp01 = v => v < 0 ? 0 : v > 1 ? 1 : v;
const sstep = (a, b, v) => { const t = clamp01((v - a) / (b - a)); return t * t * (3 - 2 * t); };
const mix = (a, b, t) => a + (b - a) * t;
const rnd = (a, b) => a + Math.random() * (b - a);
const mod = (a, n) => ((a % n) + n) % n;

/* 关键时刻（小时 → 列）0 白天 1 黄昏 2 傍晚 3 夜 */
const KEYS = [[0, 3], [1.5, 3], [2.0, 3], [2.5, 2], [3.0, 1], [3.6, 0], [10, 0], [15, 0], [17, 1], [18, 1], [19, 2], [20.3, 3], [24, 3]];
function columnsAt(h) {
  h = mod(h, 24);
  for (let i = 0; i < KEYS.length - 1; i++) {
    const [h0, c0] = KEYS[i], [h1, c1] = KEYS[i + 1];
    if (h >= h0 && h <= h1) return [c0, c1, (h - h0) / (h1 - h0)];
  }
  return [3, 3, 0];
}
const nightOf = h => { h = mod(h, 24); return clamp01(sstep(17.4, 20.2, h) + 1 - sstep(1.5, 3.6, h)); };

/* =====================================================================
   状态
   ===================================================================== */
const RM_MQ = matchMedia('(prefers-reduced-motion: reduce)');
let RM = FORCE_RM || RM_MQ.matches;
const S = {
  x: 0, v: 0, target: 0, dragging: false, dragTarget: 0, dragV: 0, lift: 0, s: 0, sv: 0,
  mode: 'fwd', hLeave: 23.4, rightClock: 21.5, atEnd: false, h: 12, night: 0, checked: false, settledArrive: true,
};
const CHAT0 = () => [{ side: 0, len: 9 }, { side: 1, len: 6 }, { side: 0, len: 12 }, { side: 1, len: 10 }];
const W = {
  t: 0, bubbles: [], spawnT: 1.0, backlog: 0, unread: 1, phoneLit: 0, buzz: 0, look: 0, lookCD: 0,
  chat: CHAT0(), codeLine: 0, codeCol: 0, elbow: 0, elbowT: 0, startle: 0, startleCD: 0, startleSeated: true,
  blink: 0, blinkT: 2.5, lampLevel: 0, lampPrev: false, flick: 99, floodQ: 0, floodT: 0, floodDone: false,
  sip: 0, sipT: 3.5, bird: null, birdT: 4, lastSt: null, prevPx: 96, walkSpeed: 0,
  dust: Array.from({ length: 7 }, () => ({ u: Math.random(), w: Math.random(), ph: Math.random() * 6.28, sp: rnd(0.02, 0.05) })),
};

/* 拨杆位置 → 时钟 */
function updateClock(dt) {
  const x = S.x;
  if (S.mode === 'fwd') {
    if (x >= 0.999) {
      if (!S.atEnd) { S.atEnd = true; S.rightClock = Math.max(S.rightClock, 21.5); }
      S.rightClock = Math.min(23.4, S.rightClock + dt * (23.4 - 21.5) / 26);
      S.h = S.rightClock;
    } else if (S.atEnd && x < 0.995) {
      S.mode = 'back'; S.hLeave = S.rightClock; S.atEnd = false; W.floodDone = false;
      S.h = mix(36, S.hLeave, x);
    } else if (S.atEnd) {
      S.h = S.rightClock;
    } else {
      S.h = 12 + 9.5 * x;
    }
  } else {
    S.h = mix(36, S.hLeave, x);
    if (x <= 0.001) { S.mode = 'fwd'; S.rightClock = 21.5; S.atEnd = false; S.h = 12; }
    else if (x >= 0.999) { S.mode = 'fwd'; S.atEnd = true; S.rightClock = S.hLeave; S.h = S.hLeave; }
  }
  S.night = nightOf(S.h);
}
function clockFor(x) {   // 不改变状态，预测某位置的时钟（读屏用）
  if (S.mode === 'fwd') return (S.atEnd && x >= 0.995) ? S.rightClock : (x >= 0.999 ? 21.5 : 12 + 9.5 * x);
  return mix(36, S.hLeave, x);
}

/* 故事时间线 */
function story() {
  const st = {};
  const H = S.h;
  if (S.mode === 'fwd' || H < 24) {
    const fwd = S.mode === 'fwd';
    const h = fwd ? H : S.hLeave;
    st.o = 1 - clamp01((h - 15.2) / 0.7);
    st.b = clamp01((h - 16) / 3.2);
    st.lamp = h >= 18.4;
    st.screen = h >= 14.7 ? 'code' : 'chat';
    st.typing = h >= 15.4;
    st.cps = h >= 21.5 ? 14 : 8;
    st.lean = h >= 21.5;
    st.reach = 0; st.hpT = 0;
    if (h < 14) st.hp = 'desk';
    else if (h < 14.35) { st.hp = 'desk'; st.reach = (h - 14) / 0.35; }
    else if (h < 14.75) { st.hp = 'hand'; st.hpT = (h - 14.35) / 0.4; }
    else st.hp = 'head';
    st.cup = 'desk'; st.cupSteam = 1 - clamp01((h - 19.5) / 3.1);
    st.seated = true; st.px = 96; st.arms = 0; st.kettle = 0;
    st.sipOK = fwd && h < 14;
    st.spawn = fwd && h < 21.5;
    st.stage = fwd ? (h < 14 ? 0 : h < 21.5 ? 1 : 2) : 3;
    st.hb = -1;
  } else {
    const hb = H - 24; st.hb = hb;
    st.o = clamp01((hb - 7.0) / 0.7);
    st.b = 1 - clamp01((hb - 1.4) / 2.0);
    st.lamp = hb < 1.7;
    st.screen = hb >= 7.7 ? 'chat' : 'code';
    st.typing = false; st.cps = 0; st.lean = false; st.reach = 0; st.hpT = 0;
    st.arms = hb < 1.3 ? (hb < 0.25 ? hb / 0.25 : hb < 1.05 ? 1 : (1.3 - hb) / 0.25) : 0;
    st.hp = hb < 0.65 ? 'head' : hb < 1.3 ? 'up' : 'desk';
    st.seated = hb < 3.8 || hb >= 7.3;
    st.px = 96; st.facing = 1; st.walking = false; st.reachK = 0;
    const SX = 158;
    if (hb >= 3.8 && hb < 4.0) { st.px = 96; st.facing = 1; }
    else if (hb >= 4.0 && hb < 4.5) { st.px = 96 + (SX - 96) * (hb - 4.0) / 0.5; st.facing = 1; st.walking = true; }
    else if (hb >= 4.5 && hb < 6.6) { st.px = SX; st.facing = 1; st.reachK = (hb < 5.3 || hb > 6.3) ? 1 : 0; }
    else if (hb >= 6.6 && hb < 7.1) { st.px = SX - (SX - 96) * (hb - 6.6) / 0.5; st.facing = -1; st.walking = true; }
    else if (hb >= 7.1 && hb < 7.3) { st.px = 96; st.facing = -1; }
    const leaveSteam = 1 - clamp01((S.hLeave - 19.5) / 3.1);
    if (hb < 3.8) { st.cup = 'desk'; st.cupSteam = leaveSteam; }
    else if (hb < 4.5) { st.cup = 'hand'; st.cupSteam = 0; }
    else if (hb < 6.6) { st.cup = 'shelf'; st.cupSteam = hb >= 5.2 ? clamp01((hb - 5.2) / 0.3) : 0; }
    else if (hb < 7.3) { st.cup = 'hand'; st.cupSteam = 1; }
    else { st.cup = 'desk'; st.cupSteam = 1; }
    st.kettle = hb < 4.6 ? 0 : hb < 4.9 ? (hb - 4.6) / 0.3 : hb < 6.6 ? 1 : 1 - clamp01((hb - 6.6) / 0.6);
    st.spawn = hb >= 7.7;
    st.sipOK = st.seated && hb >= 7.3;
    st.stage = hb < 3.8 ? 3 : hb < 7.7 ? 4 : 0;
  }
  return st;
}
const STAGES = ['可以打扰', '进入专注', '专注', '结束专注', '休息'];

/* =====================================================================
   实时世界更新（30 帧）
   ===================================================================== */
function spawnBubble(open, fast) {
  if (open) {
    const p0 = { x: rnd(30, 50), y: rnd(46, 56) }, p2 = { x: 75, y: 66 };
    W.bubbles.push({ type: 'in', t: 0, dur: fast ? 0.8 : 1.15, p0, p1: { x: (p0.x + p2.x) / 2 + 2, y: Math.min(p0.y, p2.y) - 16 }, p2 });
  } else {
    W.bubbles.push({ type: 'hit', t: 0, x0: rnd(28, 54), y0: rnd(26, 50), dx: rnd(-2, 2), hit: false });
  }
}
let CODE = [];
function codeLineAt(i) {
  while (CODE.length <= i) {
    let s = (CODE.length * 9301 + 49297) % 233280;
    const r = () => (s = (s * 9301 + 49297) % 233280) / 233280;
    const indents = [0, 2, 4, 4, 2, 4, 6, 2, 0];
    const ind = CODE.length % 9 === 0 ? 0 : indents[Math.floor(r() * indents.length)];
    const toks = []; let len = ind;
    const cols = [A2, M3, M2, S3, B2, A3, M3];
    const n = 1 + Math.floor(r() * 4);
    for (let k = 0; k < n; k++) {
      const L = 2 + Math.floor(r() * 5);
      if (len + L > 19) break;
      toks.push({ at: len, len: L, col: k === 0 ? (r() < 0.5 ? A2 : M3) : cols[Math.floor(r() * cols.length)] });
      len += L + 1;
    }
    if (!toks.length) toks.push({ at: ind, len: 3, col: M3 });
    const last = toks[toks.length - 1];
    CODE.push({ toks, len: last.at + last.len });
  }
  return CODE[i];
}
function updateWorld(dt, st) {
  W.t += dt;
  // 台灯：闪两下后亮起
  if (st.lamp && !W.lampPrev) W.flick = 0;
  W.lampPrev = st.lamp;
  if (st.lamp) {
    W.flick += dt; const f = W.flick;
    W.lampLevel = f < 0.07 ? 1 : f < 0.17 ? 0 : f < 0.24 ? 1 : f < 0.42 ? 0 : 1;
  } else W.lampLevel = 0;
  // 打字
  if (st.typing) {
    W.codeCol += st.cps * dt;
    let ln = codeLineAt(W.codeLine);
    while (W.codeCol >= ln.len + 4) { W.codeCol -= ln.len + 4; W.codeLine++; ln = codeLineAt(W.codeLine); }
    W.elbowT -= dt;
    if (W.elbowT <= 0) { W.elbow ^= 1; W.elbowT = rnd(1.2, 2.4) / st.cps; }
  }
  // 气泡
  const open = st.o > 0.5;
  if (W.floodQ > 0) {
    W.floodT -= dt;
    if (W.floodT <= 0) { spawnBubble(true, true); W.floodQ--; W.floodT = 0.14; }
  } else if (st.spawn) {
    W.spawnT -= dt;
    if (W.spawnT <= 0) { spawnBubble(open, false); W.spawnT = rnd(1.5, 2.6); }
  }
  if (st.hb >= 7.7 && !W.floodDone && st.o >= 0.99) {
    W.floodDone = true; W.floodQ = Math.max(W.backlog, 4); W.backlog = 0; W.floodT = 0;
  }
  for (const b of W.bubbles) {
    b.t += dt;
    if (b.type === 'in' && b.t >= b.dur && !b.done) {
      b.done = true;
      W.unread++; W.buzz = 0.6; W.phoneLit = 2.2;
      W.chat.push({ side: Math.random() < 0.7 ? 0 : 1, len: 5 + Math.floor(Math.random() * 11) });
      if (W.chat.length > 20) W.chat.shift();
      if (st.seated && st.hp !== 'head' && !st.typing && st.arms === 0 && W.lookCD <= 0) { W.look = 1.1; W.lookCD = 2.5; }
    }
    if (b.type === 'hit' && b.t >= 0.75 && !b.hit) { b.hit = true; W.backlog++; }
  }
  W.bubbles = W.bubbles.filter(b => b.type === 'in' ? !b.done : b.t < 2.1);
  // 手机、转头
  W.buzz = Math.max(0, W.buzz - dt); W.phoneLit = Math.max(0, W.phoneLit - dt);
  W.lookCD = Math.max(0, W.lookCD - dt);
  if (W.look > 0) { W.look -= dt; if (W.look <= 0) { W.look = 0; W.unread = 0; } }
  // 眨眼
  W.blinkT -= dt;
  if (W.blinkT <= 0) { W.blink = 0.13; W.blinkT = rnd(2, 5.5); }
  W.blink = Math.max(0, W.blink - dt);
  // 惊吓
  W.startle = Math.max(0, W.startle - dt);
  // 喝一口
  if (st.sipOK) { W.sipT -= dt; if (W.sipT <= 0) { W.sip = 1.3; W.sipT = rnd(4.5, 8); } }
  else W.sip = 0;
  W.sip = Math.max(0, W.sip - dt);
  // 鸟
  W.birdT -= dt;
  if (!W.bird && W.birdT <= 0) { W.bird = { x: 17, y: rnd(23, 31) }; W.birdT = rnd(9, 16); }
  if (W.bird) { W.bird.x += 9 * dt; W.bird.y += Math.sin(W.t * 3) * 0.05; if (W.bird.x > 66) W.bird = null; }
  // 走路速度（匆忙判断）
  W.walkSpeed = mix(W.walkSpeed, Math.abs(st.px - W.prevPx) / Math.max(dt, 1e-3), 0.3);
  W.prevPx = st.px;
  for (const d of W.dust) { d.u = mod(d.u + d.sp * dt, 1); d.ph += dt * 0.7; }
}

/* =====================================================================
   画像素世界
   ===================================================================== */
const ROWS = [96, 98, 101, 104, 108, 112, 117, 123];
const CLOUDS = [{ o: 0, sp: 0.55, y: 24, w: 5.5, h: 1.6 }, { o: 31, sp: 0.38, y: 30, w: 4, h: 1.2 }, { o: 50, sp: 0.7, y: 27, w: 3, h: 1 }];
function ext(x, y) {    // 窗外：永远是白天的颜色
  let c = M3;
  if (y > 27 && bay(x, y) < (y - 27) / 9) c = B3;
  for (const cl of CLOUDS) {
    const cx = 10 + mod(cl.o + W.t * cl.sp, 64);
    const dx = (x - cx) / cl.w, dy = (y - cl.y) / cl.h;
    if (dx * dx + dy * dy < 1 || ((x - cx - cl.w * 0.3) / (cl.w * 0.5)) ** 2 + ((y - cl.y + cl.h) / cl.h) ** 2 < 1) c = B3;
  }
  const mf = 35 + Math.round(2.5 * Math.sin(x * 0.23 + 0.5) + 2 * Math.sin(x * 0.57 + 2.1));
  if (y >= mf) c = M2;
  const nh = 42 + Math.round(2 * Math.sin(x * 0.17 + 1.3) + 1.2 * Math.sin(x * 0.45));
  if (y >= nh) c = y === nh ? S3 : S2;
  if (y >= 53) c = ((x * 7 + y * 3) % 11 === 0) ? S2 : S3;
  // 对面房子
  if (y >= 45 && y <= 48) { const hw = y - 45; if (x >= 43 - hw && x <= 49 + hw) c = y === 45 ? A1 : A0; }
  if (y >= 49 && y <= 53 && x >= 41 && x <= 51) { c = B2; if ((x === 44 || x === 45) && (y === 50 || y === 51)) c = F2; if ((x === 48 || x === 49) && y >= 51) c = A0; if (y === 49) c = B1; }
  // 两棵圆树
  const t1 = ((x - 28) ** 2 + (y - 47.5) ** 2), t2 = ((x - 55.5) ** 2 + (y - 50) ** 2);
  if (x === 28 && y >= 51 && y <= 55) c = A0;
  if (x === 55 && y >= 52 && y <= 56) c = A0;
  if (t1 < 4.8 * 4.8) c = (x - 28 < -1 && y - 47.5 < 0) || ((x - 26) ** 2 + (y - 45.5) ** 2 < 4) ? S2 : S1;
  if (t2 < 3.3 * 3.3) c = ((x - 54.5) ** 2 + (y - 48.5) ** 2 < 2.2) ? S2 : S1;
  // 偶尔飞过的鸟
  if (W.bird) {
    const bx = Math.round(W.bird.x), by = Math.round(W.bird.y), up = (Math.floor(W.t * 6) & 1);
    if (!up && ((y === by && (x === bx - 1 || x === bx + 1)) || (y === by + 1 && x === bx))) c = F2;
    if (up && ((y === by - 1 && (x === bx - 2 || x === bx + 2)) || (y === by && (x === bx - 1 || x === bx || x === bx + 1)))) c = F2;
  }
  return c;
}

function drawBubble(cx, cy, s, sq, flash, alpha, clip) {
  const w = 3 + Math.round(4 * s) + (sq ? 2 : 0), h = 2 + Math.round(2 * s) - (sq ? 1 : 0);
  const x0 = Math.round(cx - w / 2), y0 = Math.round(cy - h / 2);
  const body = flash ? A3 : B3, edge = flash ? A2 : M2;
  for (let yy = 0; yy < h; yy++) for (let xx = 0; xx < w; xx++) {
    if (w > 3 && h > 2 && (xx === 0 || xx === w - 1) && (yy === 0 || yy === h - 1)) continue;
    const X = x0 + xx, Y = y0 + yy;
    if (alpha < 1 && bay(X, Y) >= alpha) continue;
    if (clip && !clip(X, Y)) continue;
    let c = yy === h - 1 ? edge : body;
    if (w >= 7 && !sq && yy === Math.floor((h - 1) / 2) && (xx === Math.floor(w / 2) - 2 || xx === Math.floor(w / 2) || xx === Math.floor(w / 2) + 2)) c = M1;
    P(X, Y, c, 0, flash ? 4 : 0);
  }
  if (s > 0.45 && !sq) { const X = x0 + 1, Y = y0 + h; if ((alpha >= 1 || bay(X, Y) < alpha) && (!clip || clip(X, Y))) P(X, Y, edge); }
}

function steam(bx, by, a, tall, seed) {
  if (a <= 0.01) return;
  for (let i = 0; i < tall; i++) {
    const y = by - i;
    const sway = RM ? 0 : Math.round(Math.sin(W.t * 2.2 + i * 0.75 + seed) * 1.1);
    const flow = RM ? 0 : (W.t * 3 + seed) % 1;
    const al = a * (1 - i / tall) * (0.55 + 0.45 * Math.sin((i - flow * 2) * 1.3 + seed));
    const x = bx + sway;
    if (bay(x, y) < al) P(x, y, STEAM);
    if (i > 2 && bay(x + 1, y) < al * 0.5) P(x + 1, y, STEAM);
  }
}

function drawWindow(st) {
  const off = Math.round(st.o * 19);
  // 外框（3 像素白框）
  R(19, 17, 63, 65, WHITE);
  R(19, 17, 63, 17, WHITE, 3);
  // 窗外与玻璃
  for (let y = 20; y <= 62; y++) for (let x = 22; x <= 60; x++) P(x, y, ext(x, y), 0, 1);
  // 撞玻璃的气泡（在玻璃后）
  const clipG = (X, Y) => X >= 22 && X <= 60 && Y >= 20 && Y <= 62;
  for (const b of W.bubbles) if (b.type === 'hit') {
    const t = b.t;
    let s, y = b.y0, x = b.x0 + b.dx * Math.min(t, 0.75), sq = false, flash = false, al = 1;
    if (t < 0.75) s = 0.1 + 0.9 * sstep(0, 0.75, t);
    else { s = 1; flash = t < 0.83; sq = t >= 0.83 && t < 1.0; if (t >= 0.98) { const u = (t - 0.98) / 1.1; y += 8 * u * u + 2 * u; al = 1 - clamp01(u); } }
    drawBubble(x, y, s, sq, flash, al, clipG);
  }
  // 玻璃上的斜向反光（只在有玻璃的部分）
  const glazed = (y) => (y >= 20 && y <= 39) || (y >= 42 - off && y <= 60 - off);
  for (let y = 20; y <= 62; y++) if (glazed(y)) for (let x = 23; x <= 59; x++) {
    const k = (x + y) % 15; if ((k === 0 || (k === 1 && (x + y) % 30 === 1)) && x < 50) VAR(x, y, 3);
  }
  // 上扇的下横档（中梃）
  R(22, 40, 60, 41, WHITE);
  // 下扇：往上推就是开窗
  R(22, 40 - off, 60, 41 - off, WHITE); R(22, 40 - off, 60, 40 - off, WHITE, 3);
  R(22, 61 - off, 60, 62 - off, WHITE); R(22, 62 - off, 60, 62 - off, TRIM);
  R(22, 40 - off, 22, 62 - off, WHITE); R(60, 40 - off, 60, 62 - off, WHITE);
  // 内框阴影
  for (let x = 22; x <= 60; x++) VAR(x, 20, 1);
  // 窗台
  R(16, 66, 66, 67, WHITE); R(16, 66, 66, 66, WHITE, 3); R(16, 67, 66, 67, TRIM);
  for (let x = 17; x <= 65; x++) VAR(x, 68, 1);
  // 窗台上的小盆栽（开窗时被风吹得摇）
  const wind = RM ? 0 : st.o * (Math.sin(W.t * 4.3) * 0.8 + Math.sin(W.t * 7.1) * 0.5);
  R(39, 62, 43, 65, POT); R(39, 62, 43, 62, POT, 3); R(40, 65, 42, 65, POT, 1);
  const leaves = [[0, 61, LEAF], [2, 61, LEAF], [4, 61, LEAFL], [-1, 60, LEAFL], [1, 60, LEAF], [3, 60, LEAFL], [5, 60, LEAF],
    [0, 59, LEAFL], [2, 59, LEAF], [4, 59, LEAFL], [1, 58, LEAF], [3, 58, LEAFL], [-1, 58, LEAF], [2, 57, LEAFL], [5, 58, LEAFL], [0, 57, LEAF]];
  for (const [dx, y, m] of leaves) P(39 + dx + Math.round(wind * (61.5 - y) / 3), y, m);
  // 百叶窗：从上往下降，每 3 行 2 行叶片 1 行日光
  R(20, 18, 62, 19, BLINDD); R(20, 18, 62, 18, BLIND);
  const L = Math.round(st.b * 43);
  for (let r = 0; r < L; r++) {
    const y = 20 + r;
    for (let x = 21; x <= 61; x++) {
      if (r === L - 1) { P(x, y, BLINDD); continue; }
      const k = r % 3;
      if (k === 0) P(x, y, BLIND);
      else if (k === 1) P(x, y, (x === 21 || x === 61) ? BLINDD : BLIND, 1);
      else { if (x >= 22 && x <= 60) P(x, y, ext(x, y), 3, 1); else P(x, y, BLINDD); }
    }
  }
  if (L > 0) { for (let y = 20; y < 20 + L + 4 && y < 70; y++) P(57, y, TRIM); P(57, 20 + L + 4, BLINDD); }
}

function drawSunAndFloor(st) {
  // 地板：木板越往前越宽
  for (let y = 96; y < 126; y++) {
    let r = 0; for (const rr of ROWS) if (y > rr) r++;
    const isSeam = ROWS.includes(y);
    const Lp = 26 + r * 7, offx = (r * 17) % Lp;
    for (let x = -14; x < 206; x++) {
      if (isSeam) { P(x, y, SEAM); continue; }
      P(x, y, mod(x + offx, Lp) === 0 ? SEAM : FLOOR, (x * 13 + r * 7 + y * 3) % 29 === 0 ? 1 : 0);
    }
  }
  // 椭圆地毯
  for (let y = 97; y <= 111; y++) for (let x = 68; x <= 136; x++) {
    const d = Math.hypot((x - 102) / 32, (y - 104) / 6.8);
    if (d < 1) P(x, y, d > 0.84 ? RUGB : (d > 0.52 && d < 0.64) ? RUGL : RUG);
  }
  // 日光光斑（斜），百叶窗放下后变成一条条亮线
  const sun = (1 - S.night) * (st.o > 0.5 ? 1 : 0.92);
  if (sun > 0.02) {
    const Lb = Math.round(st.b * 43);
    for (let y = 97; y < 126; y++) {
      const f = (y - 97) / 28;
      const v = 62 - f * 42;                       // 对应的窗户行
      const covered = v < 20 + Lb;
      if (covered && (y - 97) % 2 === 1) continue;
      const x0 = Math.round(22 + f * 26), x1 = x0 + 37;
      for (let x = x0; x <= x1; x++) {
        const edge = Math.min(x - x0, x1 - x) / 3;
        const a = sun * Math.min(1, edge + 0.35) * (covered ? 0.9 : 1);
        if (bay(x, y) < a) VAR(x, y, 3);
      }
    }
  }
}

function drawRoom(st) {
  // 天花板 y<4，顶线 4–6
  R(-14, -14, 205, 3, CEIL);
  R(-14, 4, 205, 5, WHITE); R(-14, 4, 205, 4, WHITE, 3); R(-14, 6, 205, 6, TRIM);
  // 奶油色墙面
  R(-14, 7, 205, 73, WALL);
  for (let x = -14; x <= 205; x++) VAR(x, 7, 1);
  // 白色腰板 y74–91，每 8 像素一条缝
  for (let y = 74; y <= 91; y++) for (let x = -14; x <= 205; x++) P(x, y, (mod(x, 8) === 0 && y > 75) ? TRIM : PANEL, y === 74 ? 3 : 0);
  R(-14, 75, 205, 75, TRIM);
  // 踢脚线 92–95
  R(-14, 92, 205, 95, BASE); R(-14, 92, 205, 92, TRIM);
  drawSunAndFloor(st);
}

function drawFrame() {
  // 小画框 x76–92、y24–36
  R(76, 24, 92, 36, DSEAM);
  R(77, 25, 91, 35, WHITE);
  for (let y = 26; y <= 34; y++) for (let x = 78; x <= 90; x++) {
    let c = M3;
    if (y >= 31 - Math.round(Math.sin(x * 0.6) * 1.2)) c = S2;
    if (y >= 33) c = S1;
    if ((x - 87) ** 2 + (y - 28) ** 2 < 2.5) c = A2;
    P(x, y, c === M3 ? BK[5] : c === S2 ? BK[2] : c === S1 ? BK[7] : BK[0]);
  }
  for (let x = 77; x <= 93; x++) VAR(x, 37, 1);
}

function drawShelf(st) {
  // 书架 x162–190、y32–95
  R(162, 32, 190, 95, DFRONT);
  R(164, 34, 188, 93, SBACK);
  for (const y of [32, 47, 63, 79]) { R(162, y, 190, y + 1, DTOP); R(162, y + 1, 190, y + 1, DFRONT); for (let x = 164; x <= 188; x++) VAR(x, y + 2, 1); }
  R(162, 94, 190, 95, DFRONT);
  R(162, 32, 163, 95, DFRONT); R(189, 32, 190, 95, DFRONT); R(190, 33, 190, 95, DSEAM);
  // 书
  const books = (x0, yb, list) => {
    let x = x0;
    for (const [w, h, m, tilt] of list) {
      if (tilt) { for (let i = 0; i < h; i++) R(x + Math.floor(i / 4), yb - i, x + Math.floor(i / 4) + w - 1, yb - i, m); x += w + Math.floor(h / 4); }
      else { R(x, yb - h + 1, x + w - 1, yb, m); P(x, yb - h + 1, m, 3); if (h > 6) R(x, yb - h + 3, x + w - 1, yb - h + 3, m, 1); x += w; }
    }
  };
  books(165, 46, [[2, 10, BK[0]], [2, 11, BK[1]], [3, 9, BK[3]], [2, 10, BK[2]], [2, 12, BK[4]], [2, 9, BK[6]], [2, 11, BK[5]], [3, 10, BK[0]], [2, 8, BK[7]]]);
  books(165, 78, [[2, 12, BK[3]], [3, 11, BK[5]], [2, 13, BK[0]], [2, 10, BK[4]]]);
  // 篮子
  R(176, 70, 187, 78, BASKET); for (let y = 71; y <= 78; y += 2) R(176, y, 187, y, BASKETD); for (let x = 176; x <= 187; x += 3) P(x, 70, BASKETD); R(176, 69, 187, 69, BASKETD);
  books(165, 93, [[3, 11, BK[1]], [2, 12, BK[6]], [2, 10, BK[2]], [3, 13, BK[3]], [2, 11, BK[0]], [2, 9, BK[5]], [3, 12, BK[4]], [2, 10, BK[7]], [2, 12, BK[1]]]);
  // 第二层：水壶和豆子罐
  R(165, 56, 171, 62, KETTLE); R(166, 55, 170, 55, KETTLE); R(167, 54, 169, 54, KETTLED); R(165, 62, 171, 62, KETTLED);
  R(166, 57, 166, 60, KETTLE, 3); P(172, 57, KETTLED); P(173, 56, KETTLED); P(164, 58, KETTLED); P(163, 59, KETTLED); P(164, 60, KETTLED);
  R(176, 56, 179, 62, JAR); R(176, 58, 179, 62, BEANS); P(177, 59, BEANS, 1); P(178, 61, BEANS, 1); R(176, 55, 179, 55, DSEAM); P(176, 56, JAR, 3);
  steam(173, 55, st.kettle, 10, 1.7);
  // 顶上一盆垂着藤的绿植
  R(175, 27, 181, 31, POT); R(174, 26, 182, 26, POT, 3);
  const plant = [[173, 25], [174, 24], [176, 23], [177, 22], [178, 24], [180, 23], [181, 22], [183, 24], [184, 25], [175, 25], [179, 25], [182, 25], [176, 24], [178, 21]];
  for (const [x, y] of plant) { P(x, y, LEAF); P(x, y + 1, LEAFL); }
  const sway = RM ? 0 : Math.sin(W.t * 1.3);
  for (let i = 0; i < 20; i++) { const x = 187 + Math.round(Math.sin(i * 0.5) * 1 + (i > 10 ? sway * 0.5 : 0)); P(x, 26 + i, i % 3 ? LEAF : LEAFL); }
  for (let i = 0; i < 12; i++) { const x = 171 - Math.round(Math.sin(i * 0.6)); P(x, 27 + i, i % 3 ? LEAFL : LEAF); }
  for (let i = 0; i < 7; i++) P(184 - Math.round(i * 0.3), 28 + i, LEAF);
  if (st.cup === 'shelf') { drawCup(182, 62, st.cupSteam, 2.3); }
}

function drawCup(x0, yb, steamA, seed) {
  // 杯子 4 宽 x0..x0+3，底 yb
  R(x0, yb - 4, x0 + 3, yb, WHITE); R(x0 + 3, yb - 4, x0 + 3, yb, TRIM);
  R(x0 + 1, yb - 4, x0 + 2, yb - 4, BEANS);
  P(x0 + 4, yb - 3, WHITE); P(x0 + 5, yb - 2, WHITE); P(x0 + 4, yb - 1, WHITE);
  steam(x0 + 1, yb - 6, steamA, 8, seed);
}

function drawDesk(st) {
  // 书桌面 y70、x68–157，左侧一条桌腿，右侧三格抽屉柜
  for (let y = 73; y <= 95; y++) for (let x = 72; x <= 133; x++) VAR(x, y, 1);   // 桌下阴影
  R(68, 70, 157, 70, DTOP); R(68, 71, 157, 72, DFRONT); R(68, 72, 157, 72, DSEAM);
  R(69, 73, 71, 95, DFRONT); R(71, 73, 71, 95, DSEAM);
  R(134, 73, 156, 95, DFRONT);
  for (const y of [80, 87]) R(134, y, 156, y, DSEAM);
  R(134, 73, 134, 95, DSEAM);
  for (const y of [76, 83, 91]) R(143, y, 147, y, DTOP);
  for (let x = 66; x <= 158; x++) { VAR(x, 96, 1); VAR(x, 97, 1); }
  // 小画框已画；显示器 x113–136、y47–61，下面有支架
  R(112, 46, 137, 62, DARK); R(112, 46, 137, 46, DARK2);
  R(123, 63, 126, 67, DARK); R(124, 63, 124, 67, DARK2); R(119, 68, 130, 69, DARK); R(119, 68, 130, 68, DARK2);
  drawScreen(st);
  // 台灯 x147：底座、立杆、斜臂，灯罩 y52–55
  R(144, 68, 150, 69, LAMPB); R(145, 67, 149, 67, LAMPB);
  R(147, 50, 147, 66, LAMPB); P(147, 49, DARK);
  P(146, 50, LAMPB); P(145, 51, LAMPB); P(144, 51, LAMPB);
  const on = W.lampLevel > 0.5;
  const sh = on ? A2 : SHADE, shd = on ? A1 : SHADE;
  R(141, 52, 144, 52, sh); R(140, 53, 145, 53, sh); R(139, 54, 146, 54, sh); R(138, 55, 147, 55, shd);
  if (!on) { R(141, 52, 144, 52, SHADE, 3); R(138, 55, 147, 55, SHADE, 1); }
  if (on) { R(140, 56, 145, 56, B3, 0, 3); R(141, 57, 144, 57, A3, 0, 3); }
  // 手机 x73
  const jit = (W.buzz > 0 && !RM) ? ((Math.floor(W.t * 30) & 1) ? 1 : -1) : 0;
  const phx = 72 + jit;
  R(phx, 68, phx + 4, 69, DARK); R(phx, 68, phx + 4, 68, DARK2);
  if (W.phoneLit > 0) R(phx + 1, 68, phx + 3, 68, B3, 0, 2); else R(phx + 1, 68, phx + 3, 68, F0);
  if (W.unread > 0) { P(phx + 5, 67, A1, 0, 2); }
  // 杯子 x82
  const sipping = W.sip > 0.15 && W.sip < 1.15 && st.seated && !RM;
  if (st.cup === 'desk' && !sipping) drawCup(81, 69, st.cupSteam, 0.4);
  // 耳机放在 x140
  if (st.hp === 'desk' && st.reach < 0.95) drawHeadphonesDesk(140, 68);
}
function drawHeadphonesDesk(cx, yb) {
  R(cx - 2, yb - 5, cx + 2, yb - 5, WHITE); P(cx - 3, yb - 4, WHITE); P(cx + 3, yb - 4, WHITE);
  R(cx - 4, yb - 3, cx - 3, yb, WHITE); R(cx + 3, yb - 3, cx + 4, yb, WHITE);
  P(cx - 3, yb - 2, TRIM); P(cx + 3, yb - 2, TRIM); R(cx - 4, yb, cx - 3, yb, TRIM); R(cx + 3, yb, cx + 4, yb, TRIM);
}

function drawScreen(st) {
  const X0 = 113, X1 = 136, Y0 = 47, Y1 = 61;
  if (st.screen === 'chat') {
    R(X0, Y0, X1, Y1, B3, 0, 2);
    R(X0, Y0, X1, Y0 + 1, M2, 0, 2); P(X0 + 1, Y0, B3, 0, 2); P(X0 + 2, Y0 + 1, M3, 0, 2);
    const vis = W.chat.slice(-6);
    for (let i = 0; i < vis.length; i++) {
      const m = vis[vis.length - 1 - i];
      const y = Y1 - 1 - i * 2;
      if (y <= Y0 + 2) break;
      if (m.side === 0) R(X0 + 2, y, X0 + 2 + Math.min(m.len, 16), y, M2, 0, 2);
      else R(X1 - 2 - Math.min(m.len, 14), y, X1 - 2, y, A2, 0, 2);
    }
  } else {
    R(X0, Y0, X1, Y1, F0, 0, 2);
    R(X0, Y0, X0 + 1, Y1, F1, 0, 2);
    const cur = W.codeLine, first = Math.max(0, cur - 6);
    for (let li = first; li <= cur; li++) {
      const y = Y0 + 1 + (li - first) * 2;
      const ln = codeLineAt(li);
      const shown = li < cur ? 99 : Math.floor(W.codeCol);
      P(X0, y, F2, 0, 2);
      for (const t of ln.toks) for (let k = 0; k < t.len; k++) { const ci = t.at + k; if (ci < shown && X0 + 3 + ci <= X1 - 1) P(X0 + 3 + ci, y, t.col, 0, 2); }
      if (li === cur && st.typing !== undefined) {
        const cx = X0 + 3 + Math.min(Math.floor(W.codeCol), ln.len);
        if (Math.floor(W.t * 2.2) % 2 === 0 && cx <= X1) P(cx, y, B3, 0, 2);
      }
    }
  }
}

function drawPersonSeated(st) {
  const startled = W.startle > 0 && W.startleSeated && !RM;
  const ht = 47 - (st.lean ? 1 : 0) - (startled ? 2 : 0);
  const look = W.look > 0 && st.hp !== 'head' && !startled;
  const hx = 92 + (look ? -1 : 0);
  // 上衣：橄榄绿
  R(89, ht + 10, 103, ht + 10, SHIRT);
  R(88, ht + 11, 104, 72, SHIRT);
  R(88, ht + 11, 88, 72, SHIRTD); R(104, ht + 11, 104, 72, SHIRTD);
  R(96, ht + 13, 96, ht + 17, SHIRTD);
  R(93, ht + 10, 99, ht + 10, SHIRTD);
  // 手臂
  const u = st.arms;
  const typing = st.typing && !RM;
  if (u > 0) {
    const handY = Math.round(mix(ht + 15, ht - 6, u));
    for (const [sx, ax] of [[88, 89], [104, 103]]) {
      line(sx, ht + 12, ax + (sx < 96 ? -1 : 1) * Math.round(2 * (1 - u)), handY + 2, SHIRT);
      P(ax + (sx < 96 ? -1 : 1) * Math.round(2 * (1 - u)), handY, SKIN); P(ax + (sx < 96 ? -1 : 1) * Math.round(2 * (1 - u)), handY + 1, SKIN);
    }
  } else {
    const eL = typing ? (W.elbow ? -1 : 0) : 0, eR = typing ? (W.elbow ? 0 : -1) : 0;
    R(87, ht + 12, 87, ht + 16 + eL, SHIRT); P(86, ht + 16 + eL, SHIRTD); P(86, ht + 15 + eL, SHIRT);
    R(105, ht + 12, 105, ht + 16 + eR, SHIRT); P(106, ht + 16 + eR, SHIRTD); P(106, ht + 15 + eR, SHIRT);
  }
  // 伸手拿耳机 / 戴耳机
  if (st.reach > 0) {
    const tx = mix(105, 139, st.reach), ty = mix(ht + 14, 64, st.reach);
    line(105, ht + 12, tx, ty, SHIRT); P(tx, ty, SKIN); P(tx + 1, ty, SKIN);
  }
  if (st.hp === 'hand') {
    const t = st.hpT, tx = mix(139, 101, t), ty = mix(64, ht + 2, t);
    line(105, ht + 12, tx, ty + 2, SHIRT); P(tx, ty + 1, SKIN);
    R(tx - 2, ty - 2, tx + 2, ty - 2, WHITE); R(tx - 3, ty - 1, tx - 2, ty + 1, WHITE); R(tx + 2, ty - 1, tx + 3, ty + 1, WHITE);
  }
  // 喝一口
  const sipping = W.sip > 0.15 && W.sip < 1.15 && !RM;
  if (sipping) { line(88, ht + 12, 90, ht + 8, SHIRT); P(91, ht + 7, SKIN); R(89, ht + 4, 91, ht + 7, WHITE); P(91, ht + 4, BEANS); }
  // 脖子
  R(95, ht + 9, 97, ht + 9, SKIND);
  // 后脑：9 像素宽黑发，带一点高光
  const rows = [[2, 6], [1, 7], [0, 8], [0, 8], [0, 8], [0, 8], [0, 8], [0, 8], [1, 7]];
  rows.forEach(([a, b], r) => R(hx + a, ht + r, hx + b, ht + r, HAIR));
  P(hx + 3, ht + 1, HAIRHI); P(hx + 4, ht + 1, HAIRHI); P(hx + 2, ht + 2, HAIRHI); P(hx + 5, ht + 0, HAIRHI);
  // 两侧耳朵
  if (look) {
    R(hx, ht + 4, hx + 1, ht + 7, SKIN); P(hx - 1, ht + 5, SKIN); P(hx, ht + 8, SKIND);
    P(hx + 1, ht + 4, W.blink > 0 ? SKIN : F0);
    P(hx + 9, ht + 4, SKIN); P(hx + 9, ht + 5, SKIND);
  } else {
    P(hx - 1, ht + 4, SKIN); P(hx - 1, ht + 5, SKIND); P(hx + 9, ht + 4, SKIN); P(hx + 9, ht + 5, SKIND);
  }
  // 灰蓝色亮边（房间暗、屏幕是主要光源）
  if (S.night > 0.5 && st.screen) {
    P(hx + 7, ht + 1, M2); for (let r = 2; r <= 7; r++) P(hx + 8, ht + r, M2); P(hx + 7, ht + 8, M2);
    P(102, ht + 10, M2); P(103, ht + 10, M2); P(104, ht + 11, M2); P(104, ht + 12, M2); P(105, ht + 12, M2);
  }
  // 耳机：白色，头梁 + 两个耳罩
  if (st.hp === 'head') {
    if (startled) {          // 滑到脖子上
      R(hx, ht + 10, hx + 8, ht + 10, WHITE);
      R(hx - 2, ht + 9, hx - 1, ht + 11, WHITE); R(hx + 9, ht + 9, hx + 10, ht + 11, WHITE); P(hx - 1, ht + 10, TRIM); P(hx + 9, ht + 10, TRIM);
    } else {
      R(hx + 1, ht - 1, hx + 7, ht - 1, WHITE); P(hx, ht, WHITE); P(hx + 8, ht, WHITE); P(hx - 1, ht + 1, WHITE); P(hx + 9, ht + 1, WHITE);
      R(hx - 2, ht + 2, hx - 1, ht + 6, WHITE); R(hx + 9, ht + 2, hx + 10, ht + 6, WHITE);
      R(hx - 1, ht + 3, hx - 1, ht + 5, TRIM); R(hx + 9, ht + 3, hx + 9, ht + 5, TRIM);
    }
  } else if (st.hp === 'up') {
    const y = ht - 8;
    R(hx + 1, y, hx + 7, y, WHITE); R(hx - 1, y + 1, hx, y + 3, WHITE); R(hx + 8, y + 1, hx + 9, y + 3, WHITE);
  }
  // 惊吓："！"
  if (startled) { R(96, ht - 7, 96, ht - 5, A1, 0, 4); P(96, ht - 3, A1, 0, 4); }
}

function drawChair() {
  // 办公椅中心 x96（在人前面）
  R(90, 66, 102, 79, DARK); R(89, 67, 103, 78, DARK); R(91, 66, 101, 66, DARK2); R(89, 67, 89, 78, DARK2);
  R(92, 69, 100, 69, DARK2);
  R(86, 80, 106, 81, DARK); R(86, 80, 106, 80, DARK2);
  R(95, 82, 97, 95, DARK); R(95, 82, 95, 95, DARK2);
  R(88, 96, 104, 96, DARK); P(86, 97, DARK); P(87, 97, DARK); P(105, 97, DARK); P(106, 97, DARK); R(95, 97, 97, 97, DARK);
  P(86, 98, F0); P(106, 98, F0); P(96, 98, F0);
  for (let x = 84; x <= 108; x++) { VAR(x, 99, 1); if (x > 87 && x < 105) VAR(x, 100, 1); }
}

function drawPersonStanding(st) {
  const f = st.facing;
  const jump = (W.startle > 0 && !W.startleSeated && !RM) ? -Math.round(3 * Math.sin(Math.PI * clamp01(1 - W.startle / 0.45))) : 0;
  const px = Math.round(st.px), y0 = 42 + jump;
  const X = c => f > 0 ? px - 3 + c : px + 2 - c;
  const HEAD = ['.HHHH.', 'HHHHHH', 'HHHHHS', 'HHHSOS', 'HHESSS', 'HHSSSS', '.HSSSS', '..SSS.'];
  const hurry = W.walkSpeed > 40;
  const amp = st.walking ? (hurry ? 4 : 3) : 0;
  const ph = st.px * (hurry ? 0.5 : 0.62);
  const sw = Math.round(Math.sin(ph) * amp);
  // 后腿
  const hipY = y0 + 28;
  const leg = (dx, m) => { line(px - 1, hipY, px - 1 + dx, y0 + 56, m); line(px, hipY, px + dx, y0 + 56, m); P(px - 1 + dx, y0 + 57, F0); P(px + dx, y0 + 57, F0); P(px + dx + f, y0 + 57, F0); };
  leg(-sw * f, PANTSD);
  // 后臂
  const armSw = Math.round(Math.sin(ph) * (amp ? amp * 0.8 : 0));
  if (!(st.cup === 'hand' || st.reachK)) line(px - f, y0 + 11, px - f + armSw * f, y0 + 24, SHIRTD);
  // 身体
  R(px - 2, y0 + 9, px + 1, y0 + 9, SHIRT);
  R(px - 3, y0 + 10, px + 2, y0 + 28, SHIRT);
  R(f > 0 ? px - 3 : px + 2, y0 + 10, f > 0 ? px - 3 : px + 2, y0 + 28, SHIRTD);
  R(px - 3, y0 + 27, px + 2, y0 + 28, PANTS);
  leg(sw * f, PANTS);
  // 头
  HEAD.forEach((row, r) => { for (let c = 0; c < 6; c++) { const ch = row[c]; if (ch === '.') continue;
    let m = ch === 'H' ? HAIR : ch === 'E' ? SKIND : SKIN; if (ch === 'O') m = W.blink > 0 ? SKIN : F0;
    P(X(c), y0 + r, m); } });
  P(X(1), y0 + 1, HAIRHI);
  R(px - 1, y0 + 8, px, y0 + 8, SKIND);
  // 前臂
  if (st.cup === 'hand') {
    line(px, y0 + 11, px, y0 + 18, SHIRT); line(px, y0 + 18, px + 3 * f, y0 + 18, SHIRT); P(px + 4 * f, y0 + 18, SKIN);
    const cx = f > 0 ? px + 4 : px - 7;
    R(cx, y0 + 15, cx + 2, y0 + 18, WHITE); R(cx + 1, y0 + 15, cx + 1, y0 + 15, BEANS);
    steam(cx + 1, y0 + 13, st.cupSteam, 7, 3.1);
  } else if (st.reachK) {
    line(px, y0 + 11, px + 3 * f, y0 + 15, SHIRT); line(px + 3 * f, y0 + 15, px + 6 * f, y0 + 14, SHIRT); P(px + 7 * f, y0 + 14, SKIN);
  } else {
    line(px, y0 + 11, px - armSw * f, y0 + 24, SHIRT); P(px - armSw * f, y0 + 25, SKIN);
  }
}

function drawBubblesIn() {
  for (const b of W.bubbles) if (b.type === 'in') {
    const t = clamp01(b.t / b.dur), it = 1 - t;
    const x = it * it * b.p0.x + 2 * it * t * b.p1.x + t * t * b.p2.x;
    const y = it * it * b.p0.y + 2 * it * t * b.p1.y + t * t * b.p2.y;
    let s = 0.15 + 0.85 * sstep(0, 0.55, t);
    if (t > 0.85) s = mix(1, 0.25, (t - 0.85) / 0.15);
    drawBubble(x, y, s, false, false, 1, null);
  }
}

function lampLight() {
  const lv = W.lampLevel;
  if (lv <= 0) return;
  for (let y = 30; y <= 73; y++) for (let x = 112; x <= 162; x++) {
    const m = getM(x, y); if (m < 20) continue;
    const ok = WALLISH[m] || m === DTOP || m === DFRONT || m === WHITE || m === LAMPB || m === DSEAM;
    if (!ok) continue;
    let I = 0;
    if (y >= 56) { const w = 4 + (y - 56) * 1.15, dx = Math.abs(x - 142.5); I = Math.max(I, (1 - dx / w) * (1 - (y - 56) / 22)); }
    const r = Math.hypot((x - 142.5) / 17, (y - 55) / 12); I = Math.max(I, (1 - r) * 0.95);
    if (y >= 70 && y <= 72) { const r2 = Math.abs(x - 142) / 26; I = Math.max(I, (1 - r2) * 1.1); }
    I *= lv;
    if (I > bay(x, y) * 0.95) VAR(x, y, 2);
  }
}

function drawDust(st) {
  if (RM) return;
  const sun = (1 - S.night) * (1 - st.b * 0.6);
  if (sun < 0.25) return;
  for (const d of W.dust) {
    const u = d.u, x = Math.round(mix(40, 66, u) + Math.sin(d.ph) * 3 + d.w * 10 - 5), y = Math.round(mix(52, 96, u) + Math.cos(d.ph * 0.7) * 2);
    if ((Math.floor(W.t * 2 + d.ph) % 5) !== 0) P(x, y, B3);
  }
}

function drawWorld(st) {
  drawRoom(st);
  drawWindow(st);
  drawFrame();
  drawShelf(st);
  drawDesk(st);
  if (st.seated) { drawPersonSeated(st); drawChair(); }
  else { drawChair(); drawPersonStanding(st); }
  lampLight();
  drawDust(st);
  drawBubblesIn();
}

/* 解析：抖动选列 + 照明变体 → RGBA；同时累加光场 */
const GC = 72, CELL = WW / GC, GR = Math.ceil(WH / CELL);
const LG = new Float32Array(GC * GR * 3), LT1 = new Float32Array(GC * GR * 3), LT2 = new Float32Array(GC * GR * 3), LOUT = new Float32Array(GC * GR * 4);
const EW = [0, 0.05, 0.42, 4.0, 2.2];   // 1 天光 2 屏幕 3 台灯 4 手机/闪光
const lampInfo = { x: 0, y: 0, i: 0 };
function resolve(h) {
  const [c0, c1, tt] = columnsAt(h);
  LG.fill(0);
  let lx = 0, ly = 0, lw = 0;
  for (let Y = 0; Y < WH; Y++) for (let X = 0; X < WW; X++) {
    const i = Y * WW + X;
    const m = mat[i];
    const base = MT[m * 4 + (BAY[((Y & 3) << 2) | (X & 3)] < tt ? c1 : c0)];
    const v = vari[i];
    const idx = v === 0 ? base : v === 1 ? DKT[base] : v === 3 ? LTT[base] : LAMPI[m];
    const o = i * 4, p = idx * 3;
    RGBA[o] = PAL[p]; RGBA[o + 1] = PAL[p + 1]; RGBA[o + 2] = PAL[p + 2]; RGBA[o + 3] = 255;
    const e = emis[i];
    if (e) {
      const w = EW[e];
      const g = ((Math.floor(Y / CELL) * GC) + Math.floor(X / CELL)) * 3;
      const col = PALF[e === 3 ? A3 : idx];
      LG[g] += col[0] * w; LG[g + 1] += col[1] * w; LG[g + 2] += col[2] * w;
      if (e === 3) { lx += X * w; ly += Y * w; lw += w; }
    }
  }
  if (lw > 0) { lampInfo.x = lx / lw + 0.5; lampInfo.y = ly / lw + 0.5; }
  else if (!lampInfo.x) { lampInfo.x = 142.5 + OFF; lampInfo.y = 56 + OFF; }
  lampInfo.i = W.lampLevel;
  // 两次半径 1 + 两次半径 3 × 0.55
  const area = CELL * CELL;
  for (let k = 0; k < LG.length; k++) LG[k] /= area;
  boxBlur(LG, LT1, 1); boxBlur(LT1, LT1, 1);
  boxBlur(LG, LT2, 3); boxBlur(LT2, LT2, 3);
  for (let k = 0, n = GC * GR; k < n; k++) {
    LOUT[k * 4] = LT1[k * 3] + 0.55 * LT2[k * 3];
    LOUT[k * 4 + 1] = LT1[k * 3 + 1] + 0.55 * LT2[k * 3 + 1];
    LOUT[k * 4 + 2] = LT1[k * 3 + 2] + 0.55 * LT2[k * 3 + 2];
    LOUT[k * 4 + 3] = 1;
  }
}
const tmpB = new Float32Array(GC * GR * 3);
function boxBlur(src, dst, r) {
  const n = 2 * r + 1;
  for (let y = 0; y < GR; y++) for (let x = 0; x < GC; x++) for (let c = 0; c < 3; c++) {
    let s = 0; for (let k = -r; k <= r; k++) { const xx = Math.min(GC - 1, Math.max(0, x + k)); s += src[(y * GC + xx) * 3 + c]; }
    tmpB[(y * GC + x) * 3 + c] = s / n;
  }
  for (let y = 0; y < GR; y++) for (let x = 0; x < GC; x++) for (let c = 0; c < 3; c++) {
    let s = 0; for (let k = -r; k <= r; k++) { const yy = Math.min(GR - 1, Math.max(0, y + k)); s += tmpB[(yy * GC + x) * 3 + c]; }
    dst[(y * GC + x) * 3 + c] = s / n;
  }
}

function renderWorld(st) {
  mat.fill(0); vari.fill(0); emis.fill(0);
  drawWorld(st);
  resolve(S.h);
}

/* =====================================================================
   WebGL2 合成
   ===================================================================== */
const VS = `#version 300 es
in vec2 aP; void main(){ gl_Position = vec4(aP,0.0,1.0); }`;
const FS = `#version 300 es
precision highp float;
uniform vec2 uRes; uniform vec4 uCap; uniform float uShell;
uniform vec2 uOrigin; uniform float uK;
uniform sampler2D uWorld; uniform sampler2D uLight; uniform vec2 uLS; uniform float uCell;
uniform vec4 uLens; uniform float uLensR; uniform float uLift;
uniform float uNight; uniform vec3 uShellA; uniform vec3 uShellB;
uniform vec3 uLamp; uniform vec3 uLampCol; uniform float uTime; uniform float uLightAmt;
out vec4 outColor;
float sdSt(vec2 p, vec2 hs){ float r=hs.y; vec2 q=vec2(max(abs(p.x)-(hs.x-r),0.0), p.y); return length(q)-r; }
vec2 gSt(vec2 p, vec2 hs){ float r=hs.y; vec2 q=vec2(sign(p.x)*max(abs(p.x)-(hs.x-r),0.0), p.y); float l=length(q); return l>1e-4? q/l : vec2(0.0,-1.0); }
vec3 bsp(vec2 c){
  vec2 p=c-0.5; vec2 i=floor(p); vec2 f=p-i; vec2 f2=f*f, f3=f2*f;
  vec2 w0=(1.0-3.0*f+3.0*f2-f3)/6.0, w1=(4.0-6.0*f2+3.0*f3)/6.0, w2=(1.0+3.0*f+3.0*f2-3.0*f3)/6.0, w3=f3/6.0;
  vec2 g0=w0+w1, g1=w2+w3;
  vec2 h0=(i-0.5+w1/g0)/uLS, h1=(i+1.5+w3/g1)/uLS;
  vec3 a=texture(uLight,vec2(h0.x,h0.y)).rgb, b=texture(uLight,vec2(h1.x,h0.y)).rgb;
  vec3 cc=texture(uLight,vec2(h0.x,h1.y)).rgb, d=texture(uLight,vec2(h1.x,h1.y)).rgb;
  return g0.y*(g0.x*a+g1.x*b)+g1.y*(g0.x*cc+g1.x*d);
}
vec3 scene(vec2 fp){
  vec2 wp=(fp-uOrigin)/uK;
  ivec2 t=clamp(ivec2(floor(wp)), ivec2(0), ivec2(${WW - 1},${WH - 1}));
  vec3 c=texelFetch(uWorld,t,0).rgb;
  vec3 L=clamp(bsp(wp/uCell)*uLightAmt,0.0,1.0);
  return 1.0-(1.0-c)*(1.0-L);
}
float hash(vec2 p){ vec3 p3=fract(vec3(p.xyx)*0.1031); p3+=dot(p3,p3.yzx+33.33); return fract((p3.x+p3.y)*p3.z); }
void main(){
  vec2 fp=vec2(gl_FragCoord.x, uRes.y-gl_FragCoord.y);
  vec2 p=fp-uCap.xy;
  float capH=uCap.w*2.0;
  float dC=sdSt(p,uCap.zw);
  vec2 hsW=uCap.zw-uShell;
  float dW=sdSt(p,hsW);
  vec2 lp=uLamp.xy;
  // ---- 外面：一大一小两层柔影 + 台灯漏光 ----
  float s1=1.0-smoothstep(-0.08*capH,0.115*capH,sdSt(p-vec2(0.0,0.035*capH),uCap.zw));
  float s2=1.0-smoothstep(-0.015*capH,0.035*capH,sdSt(p-vec2(0.0,0.014*capH),uCap.zw));
  float sa=1.0-(1.0-0.16*s1)*(1.0-0.30*s2);
  float ga=uLamp.z*0.55*exp(-max(dC,0.0)/(0.075*capH))*exp(-length(fp-lp)/(0.6*capH));
  float edgeF=1.0-smoothstep(0.07*capH,0.155*capH,dC);
  ga=clamp(ga,0.0,0.6)*edgeF; sa*=edgeF;
  vec4 outside=vec4(uLampCol*ga, ga+sa*(1.0-ga));
  // ---- 外壳 ----
  float t=clamp(-dC/uShell,0.0,1.0);
  vec2 g=gSt(p,uCap.zw);
  float th=t*3.14159265;
  vec3 n=normalize(vec3(g*cos(th), sin(th)*1.15+0.05));
  vec3 base=mix(uShellA,uShellB,uNight);
  vec3 Lk=normalize(vec3(-0.45,-0.7,0.55));
  float dif=max(dot(n,Lk),0.0);
  vec3 sc=base*(0.66+0.40*dif);
  vec3 r=reflect(vec3(0.0,0.0,-1.0),n);
  vec3 S1=normalize(vec3(-0.55,-0.62,0.56));
  float box=smoothstep(0.45,0.90,dot(r,S1));
  float fill=smoothstep(0.55,1.0,dot(r,normalize(vec3(0.1,0.88,0.45))));
  float fr=0.06+0.94*pow(1.0-clamp(n.z,0.0,1.0),3.0);
  sc+= vec3(1.0,0.99,0.97)*(box*0.80+fill*0.10)*mix(0.30,1.0,fr);
  sc*= mix(1.0,0.66,smoothstep(0.5,1.0,t));             // 靠窗一侧压暗，凹陷感
  sc*= mix(1.0,0.86,smoothstep(0.35,0.0,t)*smoothstep(0.0,0.6,g.y));
  vec3 toL=vec3(lp-fp,0.55*capH); float dl=length(toL);
  sc+= uLampCol*uLamp.z*0.65*max(dot(n,toL/dl),0.0)*exp(-length(fp-lp)/(0.55*capH));
  // ---- 窗口 ----
  vec3 bg=scene(fp);
  vec2 lc=uLens.xy, lr=uLens.zw; float lmin=min(lr.x,lr.y);
  vec2 q=(fp-lc)/lr; float d=length(q);
  float so=(0.05+0.06*uLift)*uLensR, sb=(0.09+0.10*uLift)*uLensR;
  vec2 qs=(fp-lc-vec2(0.0,so))/(lr*(1.0+0.03*uLift));
  float dsd=(length(qs)-1.0)*lmin;
  bg*=1.0-(1.0-smoothstep(-sb,sb,dsd))*(0.30+0.12*uLift);
  bg*=1.0-0.40*smoothstep(-0.04*capH,0.01*capH,sdSt(p-vec2(0.0,0.03*capH),hsW));   // 上沿内阴影
  bg*=1.0-0.38*exp(dW/(0.012*capH));                                                // 一圈暗边
  float dg=(p.x*0.55+p.y)/capH;
  float hl=smoothstep(-0.62,-0.30,dg)*(1.0-smoothstep(-0.30,0.02,dg));
  bg=bg+(1.0-bg)*hl*0.075;                                                           // 大面积很淡的斜向高光
  vec3 win=bg;
  if(d<1.0+2.0/lmin){
    float sd, ds=0.0;
    if(d<0.7) sd=d/1.24;
    else { float s=(d-0.7)/0.3; sd=0.5645+0.2419*s+0.7436*s*s; ds=s*s; }
    vec2 dir=d>1e-5? q/d : vec2(0.0);
    float k=0.05*ds;
    vec3 lens;
    lens.r=scene(lc+dir*sd*(1.0-k*0.5)*lr).r;
    lens.g=scene(lc+dir*sd*lr).g;
    lens.b=scene(lc+dir*sd*(1.0+k*0.5)*lr).b;
    float rim=smoothstep(0.70,0.92,d)*(1.0-smoothstep(0.93,1.0,d));
    lens+=vec3(1.0,0.97,0.90)*rim*pow(clamp(dir.y,0.0,1.0),1.6)*0.32;            // 下沿内侧聚光
    float tr=1.0-smoothstep(0.0,0.45,length((q-vec2(-0.06,-0.60))*vec2(0.75,1.6)));
    lens=lens+(1.0-lens)*tr*0.20;                                                 // 上方柔和反光
    lens*=1.0-0.45*smoothstep(0.80,1.0,d);                                        // 边缘靠变暗显厚度
    lens*=1.0+0.03*uLift;
    float a=clamp((1.0-d)*lmin+0.5,0.0,1.0);
    win=mix(bg,lens,a);
  }
  float covC=clamp(0.5-dC,0.0,1.0), covW=clamp(0.5-dW,0.0,1.0);
  vec3 inner=mix(sc,win,covW);
  float nz=(hash(fp+fract(uTime*0.37)*311.0)-0.5)*(1.5/255.0);
  vec4 res=vec4(inner,1.0)*covC+outside*(1.0-covC);
  res.rgb+=nz*res.a;
  outColor=res;
}`;

/* =====================================================================
   组件
   ===================================================================== */
class FocusSwitch extends HTMLElement {
  connectedCallback() {
    if (this._init) return; this._init = true;
    this.canvas = document.createElement('canvas');
    this.btn = document.createElement('button');
    this.btn.type = 'button';
    this.btn.setAttribute('role', 'switch');
    this.btn.setAttribute('aria-checked', 'false');
    this.btn.setAttribute('aria-label', '专注模式');
    this.live = document.createElement('div');
    this.live.className = 'sr'; this.live.setAttribute('aria-live', 'polite'); this.live.setAttribute('role', 'status');
    this.append(this.canvas, this.btn, this.live);
    this.gl = null;
    if (!FORCE_NOGL) {
      try { this.gl = this.canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false }); } catch (e) { this.gl = null; }
      if (this.gl && !this.setupGL()) this.gl = null;
    }
    if (!this.gl) { this.ctx = this.canvas.getContext('2d'); this.wc = document.createElement('canvas'); this.wc.width = WW; this.wc.height = WH; this.wctx = this.wc.getContext('2d'); this.wimg = this.wctx.createImageData(WW, WH); }
    this.mode = this.gl ? 'webgl2' : 'canvas2d';
    this.geo = null; this.visible = true; this.running = false; this.last = 0; this.worldAcc = 1; this.worldDirty = true;
    this.lastStage = -1; this.lastNightCss = -1;
    this.bindEvents();
    this.ro = new ResizeObserver(() => { this.resize(); this.kick(); }); this.ro.observe(this);
    this.io = new IntersectionObserver(es => { for (const e of es) this.visible = e.isIntersecting; if (this.visible) this.kick(); }); this.io.observe(this);
    document.addEventListener('visibilitychange', () => { if (!document.hidden) this.kick(); });
    RM_MQ.addEventListener && RM_MQ.addEventListener('change', e => { RM = FORCE_RM || e.matches; this.kick(); });
    this.resize();
    updateClock(0);
    this.stepWorld(0);
    this.kick();
  }
  setupGL() {
    const gl = this.gl;
    const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) { console.warn(gl.getShaderInfoLog(s)); return null; } return s; };
    const vs = sh(gl.VERTEX_SHADER, VS), fs = sh(gl.FRAGMENT_SHADER, FS);
    if (!vs || !fs) return false;
    const pr = gl.createProgram(); gl.attachShader(pr, vs); gl.attachShader(pr, fs); gl.linkProgram(pr);
    if (!gl.getProgramParameter(pr, gl.LINK_STATUS)) { console.warn(gl.getProgramInfoLog(pr)); return false; }
    this.pr = pr; gl.useProgram(pr);
    const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(pr, 'aP'); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    this.u = {};
    for (const n of ['uRes', 'uCap', 'uShell', 'uOrigin', 'uK', 'uWorld', 'uLight', 'uLS', 'uCell', 'uLens', 'uLensR', 'uLift', 'uNight', 'uShellA', 'uShellB', 'uLamp', 'uLampCol', 'uTime', 'uLightAmt']) this.u[n] = gl.getUniformLocation(pr, n);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    this.texW = gl.createTexture(); gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, this.texW);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, WW, WH, 0, gl.RGBA, gl.UNSIGNED_BYTE, RGBA);
    this.texL = gl.createTexture(); gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, this.texL);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA16F, GC, GR, 0, gl.RGBA, gl.FLOAT, LOUT);
    gl.uniform1i(this.u.uWorld, 0); gl.uniform1i(this.u.uLight, 1);
    this.canvas.addEventListener('webglcontextlost', e => { e.preventDefault(); });
    return !gl.getError();
  }
  resize() {
    const rect = this.getBoundingClientRect();
    if (rect.width < 2) return;
    const dpr = window.devicePixelRatio || 1;
    const W_ = Math.max(2, Math.round(rect.width * dpr)), H_ = Math.max(2, Math.round(rect.height * dpr));
    this.canvas.width = W_; this.canvas.height = H_;
    this.canvas.style.width = (W_ / dpr) + 'px'; this.canvas.style.height = (H_ / dpr) + 'px';
    // 尺寸
    const capH = H_ / 1.32, capW = capH * 1.85, cx = W_ / 2, cy = H_ / 2;
    const shell = capH * 0.046;
    const winH = capH - 2 * shell, winW = capW - 2 * shell;
    let k = Math.max(1, Math.floor(winH / 112));
    if (1 - winH / (112 * (k + 1)) <= 0.12) k += 1;
    const lensR = winH / 2 - capH * 0.032, inset = capH * 0.032;
    this.geo = { W: W_, H: H_, dpr, capH, capW, cx, cy, shell, winH, winW, k, lensR, inset,
      ox: Math.round(cx - (WW / 2) * k), oy: Math.round(cy - (WH / 2) * k),
      lx0: cx - winW / 2 + inset + lensR, lx1: cx + winW / 2 - inset - lensR };
    const b = this.btn.style;
    b.left = ((cx - capW / 2) / dpr) + 'px'; b.top = ((cy - capH / 2) / dpr) + 'px';
    b.width = (capW / dpr) + 'px'; b.height = (capH / dpr) + 'px';
  }
  travelCss() { const g = this.geo; return g ? (g.lx1 - g.lx0) / g.dpr : 200; }
  bindEvents() {
    const btn = this.btn;
    let startX = 0, startPos = 0, startT = 0, moved = 0, lastPX = 0, lastT = 0, pid = null;
    this.suppressClick = false;
    btn.addEventListener('pointerdown', e => {
      if (e.button !== 0) return;
      pid = e.pointerId; btn.setPointerCapture(pid); btn.classList.add('drag');
      S.dragging = true; startX = e.clientX; startPos = S.x; S.dragTarget = S.x; startT = performance.now(); moved = 0;
      lastPX = e.clientX; lastT = startT; S.dragV = 0; S.v = 0;
      this.kick();
    });
    btn.addEventListener('pointermove', e => {
      if (!S.dragging || e.pointerId !== pid) return;
      const now = performance.now();
      moved = Math.max(moved, Math.abs(e.clientX - startX));
      const tr = this.travelCss();
      S.dragTarget = Math.max(0, Math.min(1, startPos + (e.clientX - startX) / tr));
      const dt = Math.max(1, now - lastT) / 1000;
      const inst = (e.clientX - lastPX) / tr / dt;
      S.dragV = mix(S.dragV, inst, 0.5);
      lastPX = e.clientX; lastT = now;
      if (Math.abs(S.dragV) > 3.4 && W.startleCD <= 0 && !RM) {
        W.startle = W.lastSt && !W.lastSt.seated ? 0.45 : 1.0; W.startleSeated = !(W.lastSt && !W.lastSt.seated); W.startleCD = 1.5;
      }
      this.kick();
    });
    const end = e => {
      if (!S.dragging || (e && e.pointerId !== pid)) return;
      S.dragging = false; btn.classList.remove('drag');
      const dur = performance.now() - startT;
      if (moved < 4 && dur < 400) { this.suppressClick = false; this.pendingTap = true; S.v = 0; }
      else {
        this.suppressClick = true;
        if (performance.now() - lastT > 80) S.dragV = 0;
        const v = Math.max(-3, Math.min(3, S.dragV));
        S.v = v;
        this.setTarget((S.x + v * 0.09 >= 0.5) ? 1 : 0);
      }
      this.kick();
    };
    btn.addEventListener('pointerup', end);
    btn.addEventListener('pointercancel', end);
    btn.addEventListener('lostpointercapture', end);
    btn.addEventListener('click', e => {
      if (this.suppressClick) { this.suppressClick = false; return; }
      this.pendingTap = false;
      this.setTarget(S.checked ? 0 : 1);
      this.kick();
    });
    btn.addEventListener('keydown', e => {
      let t = null;
      if (e.key === 'ArrowRight' || e.key === 'ArrowUp') t = Math.min(1, S.target + 0.04);
      else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') t = Math.max(0, S.target - 0.04);
      else if (e.key === 'Home') t = 0; else if (e.key === 'End') t = 1;
      if (t === null) return;
      e.preventDefault();
      this.setTarget(Math.round(t * 100) / 100, true);
      this.announce(true);
      this.kick();
    });
  }
  setTarget(t, keyboard) {
    S.target = t;
    const checked = t >= 0.5;
    if (checked !== S.checked) {
      S.checked = checked;
      this.btn.setAttribute('aria-checked', checked ? 'true' : 'false');
      this.dispatchEvent(new Event('change', { bubbles: true }));
    }
  }
  get checked() { return S.checked; }
  set checked(v) { this.setTarget(v ? 1 : 0); this.kick(); }
  announce(force) {
    const st = W.lastSt; if (!st) return;
    const stg = force ? stageAt(S.target) : st.stage;
    if (force || stg !== this.lastStage) { this.lastStage = stg; this.live.textContent = STAGES[stg]; }
  }
  physics(dt) {
    const om = RM ? 16 : 9.5;
    const px = S.x;
    if (S.dragging) {
      S.x += (S.dragTarget - S.x) * (1 - Math.exp(-34 * dt));
      S.v = (S.x - px) / Math.max(dt, 1e-4);
    } else {
      const a = om * om * (S.target - S.x) - 2 * om * S.v;
      S.v += a * dt; S.x += S.v * dt;
      if (S.x < 0) { S.x = 0; S.v = Math.max(0, S.v); }
      if (S.x > 1) { S.x = 1; S.v = Math.min(0, S.v); }
      if (Math.abs(S.target - S.x) < 1e-4 && Math.abs(S.v) < 1e-3) { S.x = S.target; S.v = 0; }
    }
    // 到端点再弹一下
    const atEnd = !S.dragging && (S.x <= 0.002 || S.x >= 0.998);
    if (atEnd && !S.settledArrive) { S.settledArrive = true; if (!RM) S.sv -= Math.min(3, Math.abs(this.arrV || 0)) * 2.2; }
    if (!atEnd) S.settledArrive = false;
    this.arrV = S.v;
    // 透镜浮起
    S.lift += ((S.dragging ? 1 : 0) - S.lift) * (1 - Math.exp(-14 * dt));
    // 欠阻尼弹簧拉伸
    if (RM) { S.s = 0; S.sv = 0; }
    else {
      const sT = Math.abs(S.v) * 0.42;
      const acc = 170 * (sT - S.s) - 11 * S.sv;
      S.sv += acc * dt; S.s += S.sv * dt;
    }
    W.startleCD = Math.max(0, W.startleCD - dt);
  }
  isSettled() {
    return !S.dragging && Math.abs(S.target - S.x) < 1e-4 && Math.abs(S.v) < 1e-3 && Math.abs(S.s) < 1e-3 && Math.abs(S.sv) < 1e-3 && Math.abs(S.lift) < 1e-3;
  }
  stepWorld(dt) {
    const st = story();
    W.lastSt = st;
    updateWorld(dt, st);
    renderWorld(st);
    this.worldDirty = true;
  }
  kick() { if (!this.running && this.visible && !document.hidden) { this.running = true; this.last = performance.now(); requestAnimationFrame(this.loop.bind(this)); } }
  loop(now) {
    if (!this.visible || document.hidden) { this.running = false; return; }   // 移出屏幕就暂停
    const dt = Math.min(0.05, Math.max(0, (now - this.last) / 1000)); this.last = now;
    this.tick(dt);
    this.draw(now / 1000);
    if (RM && this.isSettled() && !this.pendingTap && !(S.mode === 'fwd' && S.atEnd && S.rightClock < 23.4)) { this.running = false; return; }  // 减弱动效：静止时停止绘制
    requestAnimationFrame(this.loop.bind(this));
  }
  tick(dt) {
    this.physics(dt);
    updateClock(dt);
    this.worldAcc += dt;
    if (this.worldAcc >= 1 / 30) { const wdt = Math.min(0.1, this.worldAcc); this.worldAcc = 0; this.stepWorld(wdt); }
    this.announce(false);
    const nn = Math.round(S.night * 500) / 500;
    if (nn !== this.lastNightCss) { this.lastNightCss = nn; document.documentElement.style.setProperty('--night', nn.toFixed(3)); }
  }
  lensRect() {
    const g = this.geo;
    const lx = mix(g.lx0, g.lx1, S.x);
    const r = g.lensR * (1 + 0.012 * S.lift);
    return { x: lx, y: g.cy, rx: r * (1 + 0.2 * S.s), ry: r * (1 - 0.075 * S.s) };
  }
  draw(time) {
    const g = this.geo; if (!g) return;
    if (this.gl) this.drawGL(time); else this.draw2D();
  }
  drawGL(time) {
    const gl = this.gl, g = this.geo, u = this.u;
    gl.viewport(0, 0, g.W, g.H);
    if (this.worldDirty) {
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, this.texW);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, WW, WH, gl.RGBA, gl.UNSIGNED_BYTE, RGBA);
      gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, this.texL);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, GC, GR, gl.RGBA, gl.FLOAT, LOUT);
      this.worldDirty = false;
    }
    const L = this.lensRect();
    gl.uniform2f(u.uRes, g.W, g.H);
    gl.uniform4f(u.uCap, g.cx, g.cy, g.capW / 2, g.capH / 2);
    gl.uniform1f(u.uShell, g.shell);
    gl.uniform2f(u.uOrigin, g.ox, g.oy);
    gl.uniform1f(u.uK, g.k);
    gl.uniform2f(u.uLS, GC, GR);
    gl.uniform1f(u.uCell, CELL);
    gl.uniform4f(u.uLens, L.x, L.y, L.rx, L.ry);
    gl.uniform1f(u.uLensR, g.lensR);
    gl.uniform1f(u.uLift, S.lift);
    gl.uniform1f(u.uNight, S.night);
    gl.uniform3fv(u.uShellA, PALF[B3]);
    gl.uniform3fv(u.uShellB, PALF[F1]);
    gl.uniform3f(u.uLamp, g.ox + lampInfo.x * g.k, g.oy + lampInfo.y * g.k, lampInfo.i);
    gl.uniform3fv(u.uLampCol, PALF[A2]);
    gl.uniform1f(u.uTime, time % 1000);
    gl.uniform1f(u.uLightAmt, 0.85);
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }
  draw2D() {
    const c = this.ctx, g = this.geo;
    if (this.worldDirty) { this.wimg.data.set(RGBA); this.wctx.putImageData(this.wimg, 0, 0); this.worldDirty = false; }
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.clearRect(0, 0, g.W, g.H);
    const stadium = (cx, cy, w, h) => { c.beginPath(); c.roundRect(cx - w / 2, cy - h / 2, w, h, h / 2); };
    const mixc = (a, b, t) => 'rgb(' + [0, 1, 2].map(i => Math.round(mix(a[i], b[i], t) * 255)).join(',') + ')';
    c.save();
    c.shadowColor = 'rgba(0,0,0,0.22)'; c.shadowBlur = g.capH * 0.12; c.shadowOffsetY = g.capH * 0.04;
    stadium(g.cx, g.cy, g.capW, g.capH);
    c.fillStyle = mixc(PALF[B3], PALF[F1], S.night); c.fill();
    c.restore();
    c.save();
    stadium(g.cx, g.cy, g.capW, g.capH); c.clip();
    const grd = c.createLinearGradient(0, g.cy - g.capH / 2, 0, g.cy + g.capH / 2);
    grd.addColorStop(0, 'rgba(255,255,255,0.35)'); grd.addColorStop(0.5, 'rgba(255,255,255,0)'); grd.addColorStop(1, 'rgba(0,0,0,0.12)');
    c.fillStyle = grd; c.fillRect(0, 0, g.W, g.H);
    stadium(g.cx, g.cy, g.winW, g.winH); c.clip();
    c.imageSmoothingEnabled = false;
    c.drawImage(this.wc, g.ox, g.oy, WW * g.k, WH * g.k);
    const L = this.lensRect();
    c.beginPath(); c.ellipse(L.x, L.y, L.rx, L.ry, 0, 0, Math.PI * 2);
    c.fillStyle = 'rgba(255,255,255,0.20)'; c.fill();
    c.lineWidth = Math.max(1, g.capH * 0.012); c.strokeStyle = 'rgba(0,0,0,0.14)'; c.stroke();
    c.restore();
  }
}
function stageAt(x) {
  const H = clockFor(x);
  if (S.mode === 'fwd') return H < 14 ? 0 : H < 21.5 ? 1 : 2;
  if (H < 24) return 3;
  const hb = H - 24; return hb < 3.8 ? 3 : hb < 7.7 ? 4 : 0;
}
if (!customElements.get('focus-switch')) {
  customElements.define('focus-switch', FocusSwitch);
}

/* 测试钩子 */
window.__focus = {
  get el() { return document.querySelector('focus-switch'); },
  get mode() { const el = this.el; return el && el.mode; },
  state() {
    const el = this.el;
    return { x: S.x, target: S.target, h: S.h, hm: mod(S.h, 24), mode: S.mode, night: S.night, checked: S.checked, k: el && el.geo && el.geo.k, render: el && el.mode, stage: W.lastSt && STAGES[W.lastSt.stage], backlog: W.backlog, bubbles: W.bubbles.length, rm: RM, startle: +W.startle.toFixed(2) };
  },
  setX(x) {
    const el = this.el;
    if (!el) return;
    S.x = x; S.target = x; S.v = 0; S.dragTarget = x; S.checked = x >= 0.5; el.btn.setAttribute('aria-checked', S.checked ? 'true' : 'false'); el.kick();
  },
  world() { return Array.from(RGBA); },
  tick(sec, step = 1 / 30) {
    const el = this.el;
    if (!el) return;
    for (let t = 0; t < sec; t += step) el.tick(step);
    el.draw(performance.now() / 1000);
  },
};
})();
