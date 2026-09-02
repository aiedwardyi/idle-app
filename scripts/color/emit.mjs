import { ramp } from "./gen.mjs";
import {
  SEVERITY,
  ACCENT_HUE,
  THEME_HUE,
  FILL,
  TRACK,
  RING,
  sevHex,
} from "./sev.mjs";

const S = (r, step) => r[step - 1]; // 1-indexed steps, as the sources number them

// Geometry and material per theme. Not colour, but they belong with the token
// block so a theme is defined in one place.
const SHAPE = {
  glass: { radius: "22px", blur: "blur(26px) saturate(180%)" },
  bento: { radius: "16px", blur: "none" },
  minimal: { radius: "10px", blur: "blur(10px)" },
  console: { radius: "8px", blur: "none" },
};

function themeTokens(theme, mode) {
  const n = ramp(THEME_HUE[theme], { mode });
  const alpha = theme === "glass" ? 0.82 : theme === "minimal" ? 0.9 : 1;
  const surface =
    alpha === 1
      ? S(n, 2)
      : `color-mix(in srgb, ${S(n, 2)} ${Math.round(alpha * 100)}%, transparent)`;
  return [
    `  --w-surface: ${surface};`,
    `  --w-surface-2: ${S(n, 3)};`,
    `  --w-track: ${S(n, TRACK + 1)};`,
    `  --w-ring: ${S(n, RING + 1)};`,
    `  --w-hair: ${mode === "light" ? "rgba(255,255,255,0.7)" : `${S(n, 6)}`};`,
    `  --w-ink: ${S(n, 12)};`,
    `  --w-ink-2: ${S(n, 11)};`,
    `  --w-ink-3: ${S(n, 10)};`,
    `  --st-warn: ${sevHex(mode, "warn")};`,
    `  --st-serious: ${sevHex(mode, "serious")};`,
    `  --st-crit: ${sevHex(mode, "crit")};`,
    `  --w-radius: ${SHAPE[theme].radius};`,
    `  --w-blur: ${SHAPE[theme].blur};`,
    `  --w-shadow: 0 ${mode === "light" ? "18px 44px -14px" : "24px 60px -16px"} ${
      mode === "light" ? "rgba(31,31,38,0.34)" : "rgba(0,0,0,0.7)"
    };`,
  ].join("\n");
}

const accentPairs = Object.entries(ACCENT_HUE).map(([name, hue]) => {
  const l = ramp(hue, { accent: true, mode: "light" })[FILL];
  const d = ramp(hue, { accent: true, mode: "dark" })[FILL];
  return `:root[data-accent="${name}"] {\n  --accent-l: ${l};\n  --accent-d: ${d};\n}`;
});

// Glass, Bento and Minimal follow the system appearance. Console is a
// terminal look and is dark in both modes on purpose, so it is emitted from
// the dark ramp twice rather than following prefers-color-scheme.
const FIXED_DARK = new Set(["console"]);

const out = [];
out.push("/* GENERATED — see docs/COLOR.md. Do not hand-edit; regenerate. */");
out.push(":root {");
out.push(themeTokens("glass", "light"));
out.push("  --accent: var(--accent-l);");
out.push("}");
out.push("@media (prefers-color-scheme: dark) {\n  :root {");
out.push(themeTokens("glass", "dark"));
out.push("  --accent: var(--accent-d);");
out.push("  }\n}");
out.push(accentPairs.join("\n"));
for (const theme of ["bento", "minimal", "console"]) {
  const base = FIXED_DARK.has(theme) ? "dark" : "light";
  out.push(`:root[data-widget-theme="${theme}"] {`);
  out.push(themeTokens(theme, base));
  out.push("}");
  out.push(
    `@media (prefers-color-scheme: dark) {\n  :root[data-widget-theme="${theme}"] {`,
  );
  out.push(themeTokens(theme, "dark"));
  out.push("  }\n}");
}
console.log(out.join("\n"));
