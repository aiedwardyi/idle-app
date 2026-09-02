// OKLCH -> sRGB hex, with gamut clipping by chroma reduction.
const f = (x) =>
  x <= 0.0031308 ? 12.92 * x : 1.055 * Math.pow(x, 1 / 2.4) - 0.055;
function oklchToRgb(L, C, hDeg) {
  const h = (hDeg * Math.PI) / 180,
    a = C * Math.cos(h),
    b = C * Math.sin(h);
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ ** 3,
    m = m_ ** 3,
    s = s_ ** 3;
  return [
    +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}
export function hex(L, C, h) {
  let c = C;
  for (let i = 0; i < 80; i++) {
    const rgb = oklchToRgb(L, c, h);
    if (rgb.every((v) => v >= -0.0008 && v <= 1.0008)) break;
    c *= 0.96;
  }
  const rgb = oklchToRgb(L, c, h).map((v) =>
    Math.round(Math.min(1, Math.max(0, f(v))) * 255),
  );
  return "#" + rgb.map((v) => v.toString(16).padStart(2, "0")).join("");
}
const lin = (c) => {
  c /= 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const relL = (h) => {
  const [r, g, b] = h.match(/\w\w/g).map((x) => parseInt(x, 16));
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
};
export const contrast = (a, b) => {
  const [x, y] = [relL(a), relL(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

// Rule 1 (Stone / OKLab): equal perceptual steps, so lightness targets are fixed
// and only hue and chroma vary between ramps.
export const L_LIGHT = [
  0.993, 0.978, 0.957, 0.936, 0.914, 0.884, 0.845, 0.783, 0.625, 0.578, 0.505,
  0.243,
];
export const L_DARK = [
  0.178, 0.213, 0.253, 0.287, 0.318, 0.358, 0.412, 0.492, 0.625, 0.668, 0.775,
  0.955,
];
// Rule 2 (Radix): step 9 carries the highest chroma; 1-8 stay near-neutral so
// surfaces never compete with data.
const C_NEUTRAL = [
  0.004, 0.005, 0.007, 0.008, 0.009, 0.01, 0.011, 0.013, 0.02, 0.02, 0.014,
  0.012,
];
const C_ACCENT = [
  0.01, 0.016, 0.032, 0.048, 0.062, 0.076, 0.094, 0.12, 0.17, 0.165, 0.12, 0.06,
];

export function ramp(hue, { accent = false, mode = "light" } = {}) {
  const Ls = mode === "light" ? L_LIGHT : L_DARK;
  const Cs = accent ? C_ACCENT : C_NEUTRAL;
  return Ls.map((L, i) => hex(L, Cs[i], hue));
}
