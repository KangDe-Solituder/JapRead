import { computed, ref, watch } from "vue";
import { palettes } from "./palettes";
export { palettes } from "./palettes";
function read(key: string, fallback: string) {
  try {
    return localStorage.getItem(key) || fallback;
  } catch {
    return fallback;
  }
}
function persist(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {}
}
const savedTheme = read("japread.theme", "dark");
export const theme = ref(
  ["dark", "light", "system"].includes(savedTheme) ? savedTheme : "dark",
);
const media = window.matchMedia("(prefers-color-scheme: dark)");
const systemDark = ref(media.matches);
media.addEventListener("change", (e) => (systemDark.value = e.matches));
export const dark = computed(
  () =>
    theme.value === "dark" || (theme.value === "system" && systemDark.value),
);
export function toggleTheme() {
  theme.value = dark.value ? "light" : "dark";
}
const storedPalette = read("japread.palette", "forest");
// Warm paper was retired; retain a similar reading tone for existing users.
const savedPalette =
  storedPalette === "warm-paper" ? "rainy-cafe" : storedPalette;
export const palette = ref(
  palettes.some((p) => p.id === savedPalette) ? savedPalette : "forest",
);
watch(
  [palette, dark],
  () => {
    const selected =
      palettes.find((p) => p.id === palette.value) || palettes[0];
    const values = dark.value ? selected.dark : selected.light;
    const root = document.documentElement;
    root.dataset.palette = selected.id;
    ["paper", "white", "ink", "muted", "accent"].forEach((key, i) =>
      root.style.setProperty(`--${key}`, values[i]),
    );
    root.style.setProperty("--on-accent", dark.value ? values[2] : "#ffffff");
    root.style.setProperty(
      "--button-bg",
      dark.value
        ? "color-mix(in srgb, var(--accent) 30%, var(--paper))"
        : values[4],
    );
    persist("japread.palette", selected.id);
  },
  { immediate: true },
);
const savedFont = Number(read("japread.fontSize", "20"));
export const fontSize = ref(
  Number.isFinite(savedFont) && savedFont >= 16 && savedFont <= 32
    ? savedFont
    : 20,
);
const savedMotion = read("japread.motion", "normal");
export const motion = ref(
  ["normal", "fast", "slow", "off"].includes(savedMotion)
    ? savedMotion
    : "normal",
);
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const animations = new Set<Animation>();
function finishAnimations() {
  // Drop completed effects too: an animated container must not keep a stale
  // compositor layer around dialogs or fixed reader panels in WebView2.
  for (const animation of animations) animation.cancel();
  animations.clear();
}
reducedMotion.addEventListener("change", (e) => {
  if (e.matches) finishAnimations();
});
export function animatePage(element: HTMLElement | null) {
  finishAnimations();
  if (!element || motion.value === "off" || reducedMotion.matches) return;
  // Keep interactive ancestors out of compositor animations. Rapidly switching
  // settings after a modal can otherwise leave painted controls outside hit tests.
  // Animate text only; dialogs, forms and fixed panels stay in a stable layer.
  const heading = element.querySelector<HTMLElement>(
    ".page-heading h1, h2, h1",
  );
  if (!heading) return;
  let animation: Animation;
  try {
    animation = heading.animate([{ opacity: 0.25 }, { opacity: 1 }], {
      duration: (
        { fast: 120, normal: 220, slow: 420 } as Record<string, number>
      )[motion.value],
      easing: "ease-out",
    });
  } catch {
    return;
  }
  animations.add(animation);
  animation.onfinish = () => {
    animations.delete(animation);
    animation.cancel();
  };
  animation.oncancel = () => animations.delete(animation);
}
watch(
  [theme, dark],
  () => {
    document.documentElement.dataset.theme = dark.value ? "dark" : "light";
    persist("japread.theme", theme.value);
  },
  { immediate: true },
);
watch(fontSize, (value) => persist("japread.fontSize", String(value)));
watch(
  motion,
  (value) => {
    if (value === "off") finishAnimations();
    persist("japread.motion", value);
    document.documentElement.dataset.motion = value;
    document.documentElement.style.setProperty(
      "--motion-duration",
      (
        { fast: "120ms", normal: "220ms", slow: "420ms", off: "0ms" } as Record<
          string,
          string
        >
      )[value],
    );
  },
  { immediate: true },
);
