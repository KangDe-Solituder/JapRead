<script setup lang="ts">
import {
  computed,
  ref,
  watch,
  nextTick,
  onMounted,
  onBeforeUnmount,
} from "vue";
import {
  cloneProfile,
  standardProfile,
  deepProfile,
  validProfile,
  type ReadingProfile,
} from "./readingProfiles";
const props = defineProps<{ count: number; disabled: boolean }>();
const emit = defineEmits<{ run: [profile: ReadingProfile] }>();
const open = ref(false),
  editing = ref(false),
  selected = ref("standard"),
  toggle = ref<HTMLButtonElement>();
let initial = cloneProfile(standardProfile);
initial.name = "自定义";
try {
  const p = JSON.parse(
    localStorage.getItem("japread.customReadingProfile") || "null",
  );
  if (p && validProfile(p)) initial = { ...p, name: "自定义" };
} catch {}
const custom = ref(initial),
  draft = ref(cloneProfile(initial));
const choices = computed(() => [
  { id: "standard", profile: standardProfile },
  { id: "deep", profile: deepProfile },
  { id: "custom", profile: custom.value },
]);
const active = computed(
  () => choices.value.find((c) => c.id === selected.value)!.profile,
);
watch(
  () => props.disabled,
  (v) => {
    if (v) {
      open.value = false;
      editing.value = false;
    }
  },
);
const root = ref<HTMLElement>();
function outside(e: PointerEvent) {
  if (
    open.value &&
    e.target instanceof Node &&
    !root.value?.contains(e.target)
  ) {
    open.value = false;
    editing.value = false;
  }
}
onMounted(() => document.addEventListener("pointerdown", outside));
onBeforeUnmount(() => document.removeEventListener("pointerdown", outside));
function close() {
  open.value = false;
  editing.value = false;
  nextTick(() => toggle.value?.focus());
}
function edit() {
  draft.value = cloneProfile(custom.value);
  editing.value = true;
}
function selectProfile(id: string) {
  selected.value = id;
  if (id === "custom") edit();
  else close();
}
function arcPath(index: number) {
  const r = 120 + index * 48;
  const point = (degrees: number) =>
    `${r * Math.cos((degrees * Math.PI) / 180)} ${r * Math.sin((degrees * Math.PI) / 180)}`;
  return `M ${point(-110)} A ${r} ${r} 0 0 1 ${point(-20)}`;
}
function save() {
  if (!validProfile(draft.value)) return;
  custom.value = cloneProfile(draft.value);
  try {
    localStorage.setItem(
      "japread.customReadingProfile",
      JSON.stringify(custom.value),
    );
  } catch {}
  selected.value = "custom";
  editing.value = false;
}
</script>
<template>
  <div ref="root" class="profile-action" @click.stop @keydown.esc.stop="close">
    <button
      ref="toggle"
      class="button secondary profile-toggle"
      :disabled="disabled"
      :aria-expanded="open"
      aria-controls="reading-profiles"
      @click="
        open = !open;
        editing = false;
      "
    >
      <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
        <path
          d="M4 7h16M4 17h16M8 4v6m8 4v6"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
        />
      </svg>
      模式
    </button>
    <button
      class="button primary profile-run"
      :disabled="disabled || !count"
      @click="emit('run', cloneProfile(active))"
    >
      注音并解读<small>{{ count }} 段 · {{ active.name }}</small>
    </button>
    <Transition name="profile-fan">
      <div
        v-if="open && !editing"
        id="reading-profiles"
        class="profile-choices"
        aria-label="处理模式"
      >
        <svg
          class="profile-rainbow"
          viewBox="-100 -240 340 240"
          aria-label="处理模式"
        >
          <g
            v-for="(c, i) in choices"
            :key="c.id"
            class="profile-arc"
            :class="`profile-arc-${c.id}`"
            :style="{
              '--order': i,
              '--ring-color': ['#70c7ee', '#aa9cf3', '#eeacd2'][i],
            }"
            role="button"
            tabindex="0"
            :aria-label="`${c.profile.name}，注音 ${c.profile.readings.effort}，解读 ${c.profile.analysis.effort}，前后各 ${c.profile.context} 段`"
            :aria-pressed="selected === c.id"
            @click="selectProfile(c.id)"
            @keydown.enter.prevent="selectProfile(c.id)"
            @keydown.space.prevent="selectProfile(c.id)"
          >
            <title>
              {{ c.profile.name }} · 前后各 {{ c.profile.context }} 段
            </title>
            <path
              :id="`profile-arc-path-${c.id}`"
              :d="arcPath(i)"
              class="arc-band"
            />
            <text dy="4" aria-hidden="true">
              <textPath
                :href="`#profile-arc-path-${c.id}`"
                startOffset="50%"
                text-anchor="middle"
              >
                {{ c.profile.name }} · {{ c.profile.readings.effort }} /
                {{ c.profile.analysis.effort }} · {{ c.profile.context }} 段
              </textPath>
            </text>
          </g>
        </svg>
        <div class="profile-mobile-choices">
          <button
            v-for="c in choices"
            :key="c.id"
            class="button secondary profile-choice"
            :aria-pressed="selected === c.id"
            @click="selectProfile(c.id)"
          >
            <strong>{{ c.profile.name }}</strong
            ><small
              >{{ c.profile.readings.effort }} /
              {{ c.profile.analysis.effort }} · 前后各
              {{ c.profile.context }} 段</small
            >
          </button>
        </div>
      </div>
    </Transition>
    <Transition name="profile-editor">
      <section
        v-if="open && editing"
        class="profile-editor"
        aria-label="自定义档位"
      >
        <h3>自定义档位</h3>
        <p>仅用于本次段落处理；保存后可再次选用。</p>
        <fieldset v-for="task in ['readings', 'analysis'] as const" :key="task">
          <legend>{{ task === "readings" ? "注音" : "解读" }}</legend>
          <label
            >推理强度<select
              v-model="draft[task].effort"
              :aria-label="`${task === 'readings' ? '注音' : '解读'}推理强度`"
            >
              <option value="none">不推理</option>
              <option value="low">Low</option>
              <option value="high">High</option>
            </select></label
          >
          <label
            >输出上限<input
              v-model.number="draft[task].maxTokens"
              type="number"
              min="256"
              max="32768"
              step="1"
              :aria-label="`${task === 'readings' ? '注音' : '解读'}输出上限`"
          /></label>
          <label
            >超时（秒）<input
              v-model.number="draft[task].timeoutSeconds"
              type="number"
              min="15"
              max="240"
              :aria-label="`${task === 'readings' ? '注音' : '解读'}超时`"
          /></label>
        </fieldset>
        <label
          >前后各最多几段<select v-model.number="draft.context">
            <option v-for="n in 6" :key="n" :value="n - 1">
              {{ n - 1 }} 段
            </option>
          </select></label
        >
        <p>
          输出上限包含推理消耗（256–32768 tokens）；超时 15–240 秒。High
          可能更慢，不保证更准确。
        </p>
        <div class="editor-actions">
          <button class="button secondary" @click="editing = false">取消</button
          ><button
            class="button primary"
            :disabled="!validProfile(draft)"
            @click="
              save();
              close();
            "
          >
            保存并选用
          </button>
        </div>
      </section>
    </Transition>
  </div>
</template>
<style scoped>
.profile-action {
  --profile-length: 228px;
  --profile-thickness: 40px;
  display: flex;
  align-items: stretch;
  gap: 8px;
  z-index: 80;
  max-width: calc(100vw - 48px);
}
.profile-toggle {
  width: 80px;
  padding-inline: 10px;
  background: color-mix(in srgb, var(--accent) 68%, #1685cb) !important;
  border-color: color-mix(in srgb, var(--accent) 65%, #43b6ff) !important;
  color: var(--paper) !important;
  box-shadow: 0 4px 18px color-mix(in srgb, var(--accent) 28%, transparent);
  gap: 8px;
}
.profile-toggle[aria-expanded="true"] {
  filter: brightness(1.15);
}
.profile-run {
  width: var(--profile-length);
  flex-shrink: 0;
}
.profile-action small {
  display: block;
  font-size: 11px;
  opacity: 0.8;
  line-height: 1.4;
}
.profile-choices {
  position: absolute;
  left: 40px;
  top: 50%;
  width: 0;
  height: 0;
}
.profile-rainbow {
  position: absolute;
  left: -100px;
  top: -240px;
  width: 340px;
  height: 240px;
  overflow: visible;
  pointer-events: none;
}
.profile-arc {
  cursor: pointer;
  transform-origin: 0px 0px;
  transition:
    transform 0.38s cubic-bezier(0.2, 0.8, 0.2, 1),
    opacity 0.25s;
  transition-delay: calc(var(--order) * 50ms);
  outline: none;
}
.arc-band {
  fill: none;
  stroke: color-mix(in srgb, var(--ring-color) 28%, var(--paper));
  stroke-width: 36px;
  stroke-linecap: round;
  pointer-events: stroke;
  transition: stroke 0.18s;
}
.profile-arc[aria-pressed="true"] .arc-band {
  stroke: color-mix(in srgb, var(--ring-color) 62%, var(--paper));
}
.profile-arc:hover .arc-band,
.profile-arc:focus-visible .arc-band {
  stroke: color-mix(in srgb, var(--ring-color) 78%, var(--paper));
}
.profile-arc text {
  fill: var(--ink);
  font: 12px var(--sans);
  font-weight: 600;
  pointer-events: none;
}
.profile-mobile-choices {
  display: none;
}
.profile-fan-enter-active,
.profile-fan-leave-active {
  transition: opacity 0.5s;
}
.profile-fan-enter-from .profile-arc,
.profile-fan-leave-to .profile-arc {
  opacity: 0;
  transform: rotate(18deg) scale(0.5);
}
.profile-editor {
  position: absolute;
  right: 0;
  bottom: calc(100% + 12px);
  width: min(480px, calc(100vw - 48px));
  max-height: 75vh;
  overflow: auto;
  padding: 22px;
  background: var(--paper);
  color: var(--ink);
  border: 1px solid var(--line);
  border-radius: 16px;
  box-shadow: 0 16px 60px #0005;
}
.profile-editor h3 {
  margin: 0;
}
.profile-editor p {
  font-size: 12px;
  color: var(--muted);
  margin: 8px 0 14px;
}
.profile-editor fieldset {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 10px;
  border: 1px solid var(--line);
  border-radius: 10px;
  margin: 12px 0;
  padding: 12px;
}
.profile-editor label {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
}
.profile-editor input,
.profile-editor select {
  width: 100%;
  min-width: 0;
}
.editor-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.profile-editor-enter-active,
.profile-editor-leave-active {
  transition:
    opacity 0.18s,
    transform 0.22s;
}
.profile-editor-enter-from,
.profile-editor-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
@media (max-width: 700px) {
  .profile-action {
    --profile-length: min(228px, calc(100vw - 136px));
  }
  .profile-run {
    padding-inline: 8px;
    gap: 6px;
  }
  .profile-choices {
    left: auto;
    right: 0;
    top: auto;
    bottom: calc(100% + 12px);
    width: 228px;
    height: auto;
  }
  .profile-rainbow {
    display: none;
  }
  .profile-mobile-choices {
    display: grid;
    gap: 8px;
  }
  .profile-choice {
    width: 100%;
    min-height: 48px;
    gap: 10px;
  }
  .profile-choice[aria-pressed="true"] {
    border-color: var(--accent);
  }
  .profile-fan-enter-active .profile-mobile-choices,
  .profile-fan-leave-active .profile-mobile-choices {
    transition:
      transform 0.3s,
      opacity 0.3s;
  }
  .profile-fan-enter-from .profile-mobile-choices,
  .profile-fan-leave-to .profile-mobile-choices {
    opacity: 0;
    transform: translateY(14px);
  }
  .profile-editor fieldset {
    grid-template-columns: 1fr;
  }
}
@media (prefers-reduced-motion: reduce) {
  * {
    animation: none !important;
    transition: none !important;
  }
}
:global(html[data-motion="off"]) * {
  animation: none !important;
  transition: none !important;
}
</style>
