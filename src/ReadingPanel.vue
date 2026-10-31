<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { api } from "./api";
import SentenceView from "./SentenceView.vue";
import type { Annotation, Chapter, Document, GeneratedReading } from "./types";
import type { ReadingProfile } from "./readingProfiles";
const notes = ref<Annotation[]>([]),
  chosen = ref<boolean[]>([]),
  doneReadings = ref(new Set<number>()),
  doneAnalysis = ref(new Set<number>()),
  savedNotes = ref(new Set<number>()),
  phase = ref("");
const props = defineProps<{
  document: Document;
  chapter: Chapter;
  paragraphIds: number[];
  profile: ReadingProfile;
}>();
const emit = defineEmits<{ close: []; saved: [] }>();
const targets = props.chapter.sentences.filter((s) =>
  props.paragraphIds.includes(s.paragraph),
);
const targetIds = new Set(targets.map((s) => s.id));
const items = ref<GeneratedReading[]>(
  (props.chapter.generatedReadings || [])
    .filter((r) => targetIds.has(r.sentenceId))
    .map((r) => ({ ...r })),
);
const running = ref(false),
  saving = ref(false),
  stop = ref(false),
  error = ref(""),
  cursor = ref(0);
const elapsed = ref(0),
  savedMessage = ref("");
let phaseStarted = 0;
let timer: ReturnType<typeof setInterval>;
function startPhase(label: string) {
  phase.value = label;
  phaseStarted = Date.now();
  elapsed.value = 0;
}
const sentenceGroups = computed(() =>
  targets.map((sentence, index) => ({
    sentence,
    index,
    readings: items.value
      .map((reading, i) => ({ reading, i }))
      .filter(({ reading }) => reading.sentenceId === sentence.id),
    notes: notes.value
      .map((note, i) => ({ note, i }))
      .filter(({ note }) => note.sentenceId === sentence.id),
  })),
);
let alive = true;
onBeforeUnmount(() => {
  alive = false;
  stop.value = true;
  clearInterval(timer);
});
const isKanji = (c: string) => /[\p{Script=Han}々]/u.test(c);
const missing = computed(() =>
  targets.reduce(
    (n, s) =>
      n +
      Array.from(s.text).filter(
        (c, i) =>
          isKanji(c) &&
          !(s.readings || []).some((r) => r.start <= i && r.end > i) &&
          !items.value.some(
            (r) => r.sentenceId === s.id && r.start <= i && r.end > i,
          ),
      ).length,
    0,
  ),
);
function covered(paragraph: number) {
  return targets
    .filter((s) => s.paragraph === paragraph)
    .every((s) =>
      Array.from(s.text).every(
        (c, i) =>
          !isKanji(c) ||
          (s.readings || []).some((r) => r.start <= i && r.end > i) ||
          items.value.some(
            (r) => r.sentenceId === s.id && r.start <= i && r.end > i,
          ),
      ),
    );
}
async function generate() {
  if (running.value || saving.value) return;
  running.value = true;
  stop.value = false;
  error.value = "";
  savedMessage.value = "";
  timer = setInterval(() => {
    elapsed.value = Math.floor((Date.now() - phaseStarted) / 1000);
  }, 1000);
  try {
    while (cursor.value < props.paragraphIds.length && !stop.value && alive) {
      const paragraph = props.paragraphIds[cursor.value];
      if (!doneReadings.value.has(paragraph)) {
        if (!covered(paragraph)) {
          startPhase("正在补充注音");
          const result = await api.readParagraph(
            props.document.id,
            props.chapter.id,
            paragraph,
            props.profile.context,
            props.profile.readings,
          );
          if (!alive) return;
          for (const r of result.items)
            if (
              !items.value.some(
                (n) =>
                  n.sentenceId === r.sentenceId &&
                  n.start < r.end &&
                  n.end > r.start,
              )
            )
              items.value.push(r);
        }
        doneReadings.value.add(paragraph);
      }
      // A paragraph is one batch: pausing must not split its readings and analysis.
      if (!doneAnalysis.value.has(paragraph)) {
        startPhase("正在逐句解读");
        const result = await api.analyzeParagraph(
          props.document.id,
          props.chapter.id,
          paragraph,
          props.profile.context,
          props.profile.analysis,
        );
        if (!alive) return;
        if (!result.package?.annotations?.length)
          throw new Error("模型未返回解读，请重试解读步骤");
        notes.value.push(...result.package.annotations);
        chosen.value.push(...result.package.annotations.map(() => true));
        doneAnalysis.value.add(paragraph);
      }
      cursor.value++;
    }
  } catch (e) {
    error.value = `第 ${cursor.value + 1} 个所选段落${doneReadings.value.has(props.paragraphIds[cursor.value]) ? "：注音已完成，解读未完成；继续处理将只重试解读。" : "：注音未完成。"} ${String(e)}`;
  } finally {
    clearInterval(timer);
    running.value = false;
  }
}
async function save() {
  if (running.value || saving.value) return;
  saving.value = true;
  error.value = "";
  try {
    await api.saveReadings(
      props.document.id,
      props.chapter.id,
      props.document.version,
      items.value,
      props.paragraphIds,
    );
    const annotations = notes.value.filter(
      (_, i) => chosen.value[i] && !savedNotes.value.has(i),
    );
    if (annotations.length) {
      await api.importAnnotations({
        schemaVersion: 1,
        documentId: props.document.id,
        version: props.document.version,
        annotations,
      });
      notes.value.forEach((_, i) => {
        if (chosen.value[i]) savedNotes.value.add(i);
      });
    }
    emit("saved");
    savedMessage.value =
      cursor.value >= props.paragraphIds.length
        ? "注音与所选解读已保存；可关闭后阅读。"
        : "已保存当前完成的部分；解读尚未全部完成，可继续处理。";
  } catch (e) {
    emit("saved");
    error.value = "保存未全部完成（可能已保存注音），可重试：" + String(e);
  } finally {
    saving.value = false;
  }
}
onMounted(generate);
</script>
<template>
  <Teleport to="body">
    <div
      class="reading-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="注音并解读"
      @keydown.esc.stop="!running && !saving && emit('close')"
    >
      <section class="reading-panel">
        <header>
          <h2>注音并解读 · {{ chapter.title }}</h2>
          <button
            class="button secondary"
            :disabled="running || saving"
            @click="emit('close')"
          >
            关闭
          </button>
        </header>
        <p>
          {{ profile.name }}档 · 注音 {{ profile.readings.effort }} / 解读
          {{ profile.analysis.effort }} · 前后各
          {{ profile.context }} 段。保留原书注音，逐段调用当前
          LLM；检查、修改后保存，未保存的结果关闭后丢弃。
        </p>
        <p aria-live="polite">
          已处理 {{ cursor }} / {{ paragraphIds.length }} 段 ·
          {{ items.length }} 条补充注音 · {{ notes.length }} 条解读 · 尚有
          {{ missing }} 个汉字未覆盖
        </p>
        <div class="processing-status" role="status">
          <span :class="{ complete: doneReadings.size === paragraphIds.length }"
            >① 注音 {{ doneReadings.size }} / {{ paragraphIds.length }} 段</span
          >
          <span :class="{ complete: doneAnalysis.size === paragraphIds.length }"
            >② 解读 {{ doneAnalysis.size }} / {{ paragraphIds.length }} 段</span
          >
          <span v-if="running"
            >第 {{ cursor + 1 }} 个所选段落 · {{ phase }} ·
            {{ elapsed }} 秒</span
          >
        </div>
        <p v-if="running" class="phase-hint">
          注音完成后自动继续解读，无需再次点击。两步都完成后即可一起保存。
        </p>
        <div class="reading-actions">
          <button
            class="button primary"
            :disabled="running || saving || cursor >= paragraphIds.length"
            @click="generate"
          >
            {{
              running
                ? phase + "…"
                : cursor >= paragraphIds.length
                  ? "处理完成"
                  : "继续处理"
            }}
          </button>
          <button
            v-if="running"
            class="button secondary"
            @click="stop = true"
            :disabled="stop"
          >
            {{ stop ? "本段完成后暂停" : "暂停后续段落" }}
          </button>
          <button
            class="button secondary"
            :disabled="running || saving"
            @click="save"
          >
            保存当前结果
          </button>
        </div>
        <p v-if="error" role="alert" class="form-error">{{ error }}</p>
        <p v-if="savedMessage" role="status">{{ savedMessage }}</p>
        <div class="sentence-preview-list">
          <article
            v-for="group in sentenceGroups"
            :key="group.sentence.id"
            class="sentence-preview"
          >
            <div class="sentence-heading">
              第 {{ group.index + 1 }} 句
              <span>第 {{ group.sentence.paragraph + 1 }} 段</span>
            </div>
            <p class="preview-original" lang="ja">
              <SentenceView
                :sentence="group.sentence"
                :annotations="[]"
                active=""
                :ruby="true"
                :generated-readings="group.readings.map((r) => r.reading)"
              />
            </p>
            <div class="sentence-analysis">
              <h4>解读</h4>
              <label
                v-for="{ note, i } in group.notes"
                :key="i"
                class="combined-note"
              >
                <input
                  type="checkbox"
                  v-model="chosen[i]"
                  :disabled="saving || savedNotes.has(i)"
                  :aria-label="`保存第 ${group.index + 1} 句解读`"
                />
                <span
                  ><small v-if="savedNotes.has(i)" class="saved-label"
                    >已保存</small
                  ><strong v-if="note.quote !== group.sentence.text">{{
                    note.quote
                  }}</strong>
                  <p>{{ note.meaning }}</p>
                  <p class="note-explanation">{{ note.explanation }}</p></span
                >
              </label>
              <p v-if="!group.notes.length" class="phase-hint">
                {{
                  doneAnalysis.has(group.sentence.paragraph)
                    ? "此句暂无解读"
                    : running &&
                        paragraphIds[cursor] === group.sentence.paragraph &&
                        doneReadings.has(group.sentence.paragraph)
                      ? "正在结合上下文解读…"
                      : "等待解读"
                }}
              </p>
            </div>
            <details
              class="sentence-readings"
              :open="group.readings.length > 0"
            >
              <summary>
                补充注音 · {{ group.readings.length }} 项
                <small>可校对读音</small>
              </summary>
              <div class="reading-list">
                <div
                  v-for="{ reading: r, i } in group.readings"
                  :key="`${r.sentenceId}:${r.start}`"
                  class="reading-item"
                >
                  <label
                    ><span lang="ja">{{ r.quote }}</span
                    ><input
                      v-model="r.reading"
                      :aria-label="`${r.quote}的假名读音`"
                      :disabled="running || saving"
                  /></label>
                  <button
                    class="text-button remove-reading"
                    :disabled="running || saving"
                    :aria-label="`移除${r.quote}的补充注音`"
                    @click="items.splice(i, 1)"
                  >
                    ×
                  </button>
                </div>
              </div>
              <p v-if="!group.readings.length" class="phase-hint">
                {{
                  doneReadings.has(group.sentence.paragraph)
                    ? "此句无需补充注音，原书注音保留。"
                    : "等待补充注音，原书注音保留。"
                }}
              </p>
            </details>
          </article>
        </div>
      </section>
    </div>
  </Teleport>
</template>
<style scoped>
.combined-note {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 0;
}
.combined-note input {
  width: auto;
  margin-top: 6px;
}
.combined-note p {
  margin: 6px 0;
}
.combined-note > span {
  min-width: 0;
}
.note-explanation {
  color: var(--muted);
  font-size: 14px;
  line-height: 1.8;
  white-space: pre-line;
}
.processing-status {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 16px 0 8px;
  font-size: 13px;
}
.processing-status span {
  background: var(--white);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 7px 10px;
}
.processing-status .complete {
  color: var(--accent);
  border-color: var(--accent);
}
.phase-hint {
  color: var(--muted);
  font-size: 13px;
}
.sentence-preview-list {
  display: grid;
  gap: 16px;
  margin-top: 20px;
}
.sentence-preview {
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 20px;
  background: var(--white);
}
.sentence-heading {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--muted);
}
.preview-original {
  font-size: 20px;
  line-height: 2.3;
  margin: 14px 0;
  overflow-wrap: anywhere;
}
.sentence-analysis {
  border-top: 1px solid var(--line);
  padding-top: 12px;
}
.sentence-analysis h4 {
  margin: 0;
  color: var(--accent);
  font-size: 13px;
}
.sentence-readings {
  margin-top: 12px;
  border-top: 1px solid var(--line);
  padding-top: 12px;
}
.sentence-readings summary {
  cursor: pointer;
  font-size: 13px;
  color: var(--muted);
}
.sentence-readings summary small {
  display: inline;
  margin-left: 10px;
}
.saved-label {
  color: var(--accent);
}
.reading-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: #0008;
  display: grid;
  place-items: center;
  padding: 24px;
}
.reading-panel {
  background: var(--paper);
  color: var(--ink);
  width: min(900px, 100%);
  max-height: 90vh;
  overflow: auto;
  padding: 24px;
  border-radius: 16px;
}
header,
.reading-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}
header {
  justify-content: space-between;
}
.reading-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin-top: 12px;
}
.reading-item {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 8px;
}
.reading-list label {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr);
  gap: 8px;
  align-items: center;
  flex: 1;
  min-width: 0;
  font-size: 14px;
}
.reading-item input {
  min-width: 0;
  padding: 7px 9px;
  font-size: 14px;
}
.remove-reading {
  padding: 6px;
  font-size: 18px;
}
small {
  display: block;
  opacity: 0.65;
  font-size: 12px;
}
input {
  width: 100%;
}
@media (max-width: 600px) {
  .reading-list {
    grid-template-columns: 1fr;
  }
  .reading-overlay {
    padding: 12px;
  }
  .reading-panel {
    padding: 16px;
  }
  .sentence-preview {
    padding: 14px;
  }
}
</style>
