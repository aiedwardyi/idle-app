import { hex, ramp, contrast } from "./gen.mjs";

// Severity gets explicit lightness targets rather than borrowing the neutral
// step ladder: amber is the lightest hue on the wheel and cannot clear 3:1
// against a light track at the same step as red.
export const SEVERITY = {
  light: {
    warn: [85, 0.565, 0.13],
    serious: [45, 0.515, 0.16],
    crit: [23, 0.46, 0.17],
  },
  dark: {
    warn: [85, 0.7, 0.14],
    serious: [45, 0.745, 0.16],
    crit: [23, 0.8, 0.14],
  },
};
export const ACCENT_HUE = { blue: 250, indigo: 282, teal: 195, magenta: 330 };
export const THEME_HUE = { glass: 285, bento: 75, minimal: 250, console: 155 };

const lin = (c) => {
  c /= 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const relL = (h) => {
  const [r, g, b] = h.match(/\w\w/g).map((x) => parseInt(x, 16));
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
};

export const FILL = 9,
  TRACK = 3,
  RING = 6;
export const sevHex = (mode, key) => {
  const [h, L, C] = SEVERITY[mode][key];
  return hex(L, C, h);
};

if (process.argv[2] === "check") {
  let fails = 0;
  for (const mode of ["light", "dark"]) {
    const n = ramp(THEME_HUE.glass, { mode }),
      track = n[TRACK],
      surface = n[1];
    const sev = ["warn", "serious", "crit"].map((k) => [k, sevHex(mode, k)]);
    const seq = sev.map(([, v]) => relL(v));
    const mono =
      mode === "light"
        ? seq.every((v, i) => i === 0 || v < seq[i - 1])
        : seq.every((v, i) => i === 0 || v > seq[i - 1]);
    console.log(`\n=== ${mode} ===  monotonic: ${mono ? "PASS" : "FAIL"}`);
    if (!mono) fails++;
    for (const [k, v] of sev) {
      const t = contrast(v, track),
        s = contrast(v, surface);
      const ok = t >= 3 && s >= 3;
      if (!ok) fails++;
      console.log(
        `  ${k.padEnd(8)} ${v}  track ${t.toFixed(2)}  surface ${s.toFixed(2)}  ${ok ? "PASS" : "FAIL"}`,
      );
    }
    for (const [k, h] of Object.entries(ACCENT_HUE)) {
      const v = ramp(h, { accent: true, mode })[FILL];
      const t = contrast(v, track),
        s = contrast(v, surface);
      const ok = t >= 3 && s >= 3;
      if (!ok) fails++;
      console.log(
        `  accent ${k.padEnd(8)} ${v}  track ${t.toFixed(2)}  surface ${s.toFixed(2)}  ${ok ? "PASS" : "FAIL"}`,
      );
    }
    const ink12 = contrast(n[11], surface),
      ink11 = contrast(n[10], surface);
    console.log(
      `  text primary ${n[11]} ${ink12.toFixed(2)} ${ink12 >= 4.5 ? "PASS" : "FAIL"}  secondary ${n[10]} ${ink11.toFixed(2)} ${ink11 >= 4.5 ? "PASS" : "FAIL"}`,
    );
    if (ink12 < 4.5 || ink11 < 4.5) fails++;
  }
  console.log(`\n${fails === 0 ? "ALL GATES PASS" : fails + " FAILURES"}`);
}
