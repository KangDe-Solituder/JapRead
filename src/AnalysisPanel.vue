<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { api } from "./api";
import type { AnalysisTask, Annotation, Document } from "./types";

const props = defineProps<{ document: Document; initialChapterId: string }>();
const emit = defineEmits<{ close: []; saved: []; settings: [] }>();
const dialog = ref<HTMLDialogElement>();
const errorMessage = ref<HTMLElement>();
const selected = ref([props.initialChapterId]);
const configured = ref(false);
const checking = ref(true);
const busy = ref(false);
const saving = ref(false);
const error = ref("");
const model = ref("");
const task = ref<AnalysisTask>();
const chapterSelected = ref<Record<string, boolean>>({});
const noteSelected = ref<Record<string, boolean>>({});
let pollTimer: ReturnType<typeof setInterval> | undefined;
let polling = false;

const characters = computed(() =>
  props.document.chapters
    .filter((chapter) => selected.value.includes(chapter.id))
    .reduce(
      (total, chapter) =>
        total +
        chapter.sentences.reduce(
          (sum, sentence) => sum + Array.from(sentence.text).length,
          0,
        ),
      0,
    ),
);
const results = computed(() => task.value?.chapterResults || []);
function chapterSaved(id: string) {
  return task.value?.savedChapterIds.includes(id) ?? false;
}
const chosenChapters = computed(() =>
  results.value
    .filter(
      (result) =>
        chapterSelected.value[result.chapterId] &&
        !chapterSaved(result.chapterId),
    )
    .map((result) => result.chapterId),
);
const chosenNotes = computed(() =>
  results.value
    .filter(
      (result) =>
        chapterSelected.value[result.chapterId] &&
        !chapterSaved(result.chapterId),
    )
    .flatMap((result) => result.package.annotations)
    .filter((note) => noteSelected.value[noteKey(note)]),
);
const totalUsage = computed(() =>
  results.value.reduce(
    (usage, result) => ({
      input: usage.input + (result.inputTokens || 0),
      output: usage.output + (result.outputTokens || 0),
    }),
    { input: 0, output: 0 },
  ),
);
const statusText = computed(() => {
  switch (task.value?.status) {
    case "queued":
      return "等待开始…";
    case "running":
      return "正在按章节解析，关闭面板也会继续。";
    case "paused":
      return "已暂停；若暂停时已有请求，会在该章完成后停止。";
    case "failed":
      return "本任务已停止，已完成章节仍保留，可继续未完成的章节。";
    case "completed":
      return "全部章节解析完成，请检查并保存。";
    case "saved":
      return "章节理解与所选解读已保存。";
    default:
      return "";
  }
});

function noteKey(note: Annotation) {
  return `${note.sentenceId}:${note.start}:${note.end}:${note.kind}`;
}
function chapterTitle(chapterId: string) {
  return (
    props.document.chapters.find((chapter) => chapter.id === chapterId)
      ?.title || "未知章节"
  );
}
function context(note: Annotation) {
  return props.document.chapters
    .flatMap((chapter) => chapter.sentences)
    .find((sentence) => sentence.id === note.sentenceId)?.text;
}
function syncSelections(value: AnalysisTask | undefined) {
  for (const result of value?.chapterResults || []) {
    if (chapterSelected.value[result.chapterId] === undefined)
      chapterSelected.value[result.chapterId] = true;
    for (const note of result.package.annotations) {
      if (noteSelected.value[noteKey(note)] === undefined)
        noteSelected.value[noteKey(note)] = true;
    }
  }
}
watch(task, syncSelections, { deep: true });
watch(error, async (message) => {
  if (!message) return;
  await nextTick();
  errorMessage.value?.scrollIntoView({ block: "center" });
});
async function refreshTask() {
  if (!task.value || polling) return;
  polling = true;
  try {
    task.value = await api.getAnalysisTask(task.value.id);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    stopPolling();
  } finally {
    polling = false;
  }
}
function startPolling() {
  stopPolling();
  pollTimer = setInterval(refreshTask, 1200);
}
function stopPolling() {
  if (pollTimer !== undefined) clearInterval(pollTimer);
  pollTimer = undefined;
}
async function perform(action: () => Promise<void>) {
  if (busy.value || saving.value) return;
  busy.value = true;
  error.value = "";
  try {
    await action();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}
async function createTask() {
  if (!selected.value.length) return;
  await perform(async () => {
    chapterSelected.value = {};
    noteSelected.value = {};
    task.value = await api.createAnalysisTask(
      props.document.id,
      selected.value,
    );
    startPolling();
  });
}
async function pauseTask() {
  if (!task.value) return;
  const id = task.value.id;
  await perform(async () => {
    task.value = await api.pauseAnalysisTask(id);
  });
}
async function resumeTask() {
  if (!task.value) return;
  const id = task.value.id;
  await perform(async () => {
    task.value = await api.resumeAnalysisTask(id);
    startPolling();
  });
}
async function deleteTask() {
  if (!task.value) return;
  const id = task.value.id;
  await perform(async () => {
    await api.deleteAnalysisTask(id);
    stopPolling();
    task.value = undefined;
    chapterSelected.value = {};
    noteSelected.value = {};
  });
}
async function save() {
  if (!task.value || !chosenChapters.value.length || saving.value) return;
  saving.value = true;
  error.value = "";
  try {
    task.value = await api.saveAnalysisTask(
      task.value.id,
      chosenChapters.value,
      {
        schemaVersion: 1,
        documentId: props.document.id,
        version: props.document.version,
        annotations: chosenNotes.value,
      },
    );
    for (const id of task.value.savedChapterIds)
      chapterSelected.value[id] = false;
    emit("saved");
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}
onMounted(async () => {
  dialog.value?.showModal();
  try {
    const [settings, tasks] = await Promise.all([
      api.settings(),
      api.listAnalysisTasks(props.document.id),
    ]);
    configured.value = !!settings.llm.endpoint && !!settings.llm.model;
    model.value = settings.llm.model;
    task.value = tasks.find((item) => item.status !== "saved") || tasks[0];
    if (task.value) startPolling();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    checking.value = false;
  }
});
onBeforeUnmount(stopPolling);
</script>

<template>
  <dialog
    ref="dialog"
    class="analysis-dialog"
    aria-labelledby="analysis-title"
    @cancel.prevent="!saving && emit('close')"
  >
    <div class="dialog-heading">
      <div>
        <span class="eyebrow">READ THE WHOLE CHAPTER</span>
        <h2 id="analysis-title">读完一章，再看懂一章</h2>
      </div>
      <button
        class="close-button"
        aria-label="关闭解析面板"
        :disabled="saving"
        @click="emit('close')"
      >
        ×
      </button>
    </div>
    <p class="muted">
      {{ document.title }} ·
      {{ checking ? "正在读取配置…" : configured ? model : "尚未连接语言模型" }}
    </p>
    <div v-if="!checking && !configured" class="analysis-empty">
      <p>先在设置中保存你的接口与模型，再回来解析。</p>
      <button class="button primary" @click="emit('settings')">前往设置</button>
    </div>
    <template v-else-if="configured">
      <template v-if="!task">
        <fieldset class="chapter-picker" :disabled="busy">
          <legend>选择章节</legend>
          <label v-for="chapter in document.chapters" :key="chapter.id">
            <input type="checkbox" v-model="selected" :value="chapter.id" />
            {{ chapter.title }}
            <small>{{ chapter.sentences.length }} 句</small>
          </label>
        </fieldset>
        <p class="muted preference-note">
          所选正文约
          {{ characters.toLocaleString() }}
          字。每次只发送一个完整章节，同时生成主旨、概述、要点和最多 30
          条精选词句；单章最多 12000 字，超长章节请先拆分导入。
        </p>
        <p class="muted preference-note">
          任务按章节保存进度。关闭面板或重启后可继续；失败不会自动重试，避免重复计费。
        </p>
        <div class="settings-actions">
          <button
            class="button primary"
            :disabled="busy || !selected.length"
            @click="createTask"
          >
            {{ busy ? "正在创建…" : "开始章节解析" }}
          </button>
        </div>
      </template>
      <template v-else>
        <div class="analysis-task-heading">
          <div>
            <strong>{{ statusText }}</strong>
            <p class="muted">
              已完成 {{ results.length }} / {{ task.chapterIds.length }} 章
              <span v-if="results.length">
                · 输入 {{ totalUsage.input || "未报告" }} / 输出
                {{ totalUsage.output || "未报告" }} tokens</span
              >
            </p>
          </div>
          <span class="tag">{{ task.status }}</span>
        </div>
        <progress
          :value="results.length"
          :max="task.chapterIds.length"
          aria-label="章节解析进度"
        ></progress>
        <div class="settings-actions">
          <button
            v-if="['queued', 'running'].includes(task.status)"
            class="button secondary"
            :disabled="busy"
            @click="pauseTask"
          >
            当前章节后暂停
          </button>
          <button
            v-if="['paused', 'failed'].includes(task.status)"
            class="button primary"
            :disabled="busy"
            @click="resumeTask"
          >
            继续未完成章节
          </button>
          <button
            v-if="task.status === 'saved'"
            class="button secondary"
            :disabled="busy"
            @click="task = undefined"
          >
            新建章节任务
          </button>
          <button
            v-if="!['queued', 'running'].includes(task.status)"
            class="text-button"
            :disabled="busy || saving"
            @click="deleteTask"
          >
            清理任务记录
          </button>
        </div>
        <p v-if="task.error" role="alert" class="form-error preference-message">
          {{ task.error }}
        </p>
        <section
          v-for="result in results"
          :key="result.chapterId"
          class="chapter-analysis-result"
        >
          <label class="chapter-analysis-heading">
            <input
              type="checkbox"
              v-model="chapterSelected[result.chapterId]"
              :disabled="
                saving ||
                task.status === 'saved' ||
                chapterSaved(result.chapterId)
              "
            />
            <span>
              <small
                >{{ chapterTitle(result.chapterId)
                }}{{ chapterSaved(result.chapterId) ? " · 已保存" : "" }}</small
              >
              <h3>{{ result.analysis.mainIdea }}</h3>
            </span>
          </label>
          <p>{{ result.analysis.overview }}</p>
          <ul class="chapter-key-points">
            <li v-for="point in result.analysis.keyPoints" :key="point">
              {{ point }}
            </li>
          </ul>
          <p class="muted">
            {{ Math.max(1, Math.round(result.elapsedMs / 1000)) }} 秒 ·
            {{ result.package.annotations.length }} 条精选解读
          </p>
          <details v-if="result.warnings.length" class="batch-history" open>
            <summary>{{ result.warnings.length }} 条未通过校验</summary>
            <ul>
              <li v-for="warning in result.warnings" :key="warning">
                {{ warning }}
              </li>
            </ul>
          </details>
          <div
            v-if="result.package.annotations.length"
            class="analysis-results"
          >
            <label
              v-for="note in result.package.annotations"
              :key="noteKey(note)"
              class="analysis-result"
            >
              <input
                type="checkbox"
                v-model="noteSelected[noteKey(note)]"
                :disabled="
                  saving ||
                  task.status === 'saved' ||
                  chapterSaved(result.chapterId) ||
                  !chapterSelected[result.chapterId]
                "
              />
              <div>
                <div class="analysis-quote">
                  <strong lang="ja">{{ note.quote }}</strong>
                  <span>{{ note.reading }}</span
                  ><small>{{ note.kind }}</small>
                </div>
                <p>{{ note.meaning }}</p>
                <p class="muted">{{ note.explanation }}</p>
                <blockquote lang="ja">{{ context(note) }}</blockquote>
              </div>
            </label>
          </div>
        </section>
      </template>
    </template>
    <p
      ref="errorMessage"
      v-if="error"
      role="alert"
      class="form-error preference-message"
    >
      {{ error }}
    </p>
    <div class="analysis-footer">
      <span class="muted">
        {{
          task
            ? "任务与逐章预览已持久保存；勾选后才会进入书库。"
            : "解析会产生模型费用，结果先预览再保存。"
        }}
      </span>
      <button
        v-if="task && results.length && task.status !== 'saved'"
        class="button primary"
        :disabled="busy || saving || !chosenChapters.length"
        @click="save"
      >
        {{
          saving
            ? "正在保存…"
            : `保存 ${chosenChapters.length} 章与 ${chosenNotes.length} 条解读`
        }}
      </button>
      <button
        class="button secondary"
        :disabled="saving"
        @click="emit('close')"
      >
        关闭
      </button>
    </div>
  </dialog>
</template>
