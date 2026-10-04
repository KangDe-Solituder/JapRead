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
import type { Annotation, Document } from "./types";
const props = defineProps<{ document: Document; initialChapterId: string }>();
const emit = defineEmits<{ close: []; saved: []; settings: [] }>();
const dialog = ref<HTMLDialogElement>();
const errorMessage = ref<HTMLElement>();
const selected = ref([props.initialChapterId]);
const configured = ref(false),
  checking = ref(true),
  busy = ref(false),
  saving = ref(false),
  stopping = ref(false);
const error = ref(""),
  status = ref(""),
  model = ref("");
watch(error, async (message) => {
  if (!message) return;
  await nextTick();
  // Center the alert so the sticky save footer cannot cover it.
  errorMessage.value?.scrollIntoView({ block: "center" });
});
const notes = ref<{ note: Annotation; selected: boolean }[]>([]);
const jobs = ref<{ chapterId: string; sentenceIds: string[] }[]>([]),
  done = ref(0);
const chosen = computed(() => notes.value.filter((n) => n.selected));
const batchSize = ref(2),
  outputLimit = ref(0),
  timeoutSeconds = ref(0);
const lastUsage = ref("");
const elapsedSeconds = ref(0);
const batchResults = ref<
  { batch: number; count: number; seconds: number; warnings: string[] }[]
>([]);
const skipped = computed(() =>
  batchResults.value.reduce((sum, batch) => sum + batch.warnings.length, 0),
);
let timer: ReturnType<typeof setInterval> | undefined;
function stopTimer() {
  if (timer !== undefined) clearInterval(timer);
  timer = undefined;
}
onBeforeUnmount(stopTimer);
const characters = computed(() =>
  props.document.chapters
    .filter((c) => selected.value.includes(c.id))
    .reduce(
      (n, c) =>
        n + c.sentences.reduce((sum, s) => sum + Array.from(s.text).length, 0),
      0,
    ),
);
function buildJobs() {
  const result: typeof jobs.value = [];
  for (const chapter of props.document.chapters.filter((c) =>
    selected.value.includes(c.id),
  )) {
    let ids: string[] = [],
      count = 0;
    for (const sentence of chapter.sentences) {
      const size = Array.from(sentence.text).length;
      if (size > 4000)
        throw new Error(
          `「${chapter.title}」有超过 4000 字的单句，暂请使用手工或 JSON 解读。`,
        );
      if (
        ids.length >= Math.max(1, Math.min(8, batchSize.value)) ||
        count + size > 4000
      ) {
        result.push({ chapterId: chapter.id, sentenceIds: ids });
        ids = [];
        count = 0;
      }
      ids.push(sentence.id);
      count += size;
    }
    if (ids.length) result.push({ chapterId: chapter.id, sentenceIds: ids });
  }
  return result;
}
async function run() {
  if (busy.value || saving.value) return;
  error.value = "";
  status.value = "";
  stopping.value = false;
  busy.value = true;
  try {
    if (!jobs.value.length) jobs.value = buildJobs();
    while (done.value < jobs.value.length && !stopping.value) {
      const job = jobs.value[done.value];
      const started = Date.now();
      elapsedSeconds.value = 0;
      timer = setInterval(() => {
        elapsedSeconds.value = Math.floor((Date.now() - started) / 1000);
      }, 1000);
      let result;
      try {
        result = await api.analyze(
          props.document.id,
          job.chapterId,
          job.sentenceIds,
        );
      } finally {
        stopTimer();
      }
      batchResults.value.push({
        batch: done.value + 1,
        count: result.package.annotations.length,
        seconds: Math.max(1, Math.round(result.elapsedMs / 1000)),
        warnings: result.warnings || [],
      });
      lastUsage.value = `最近一批：输入 ${result.usage.inputTokens ?? "未报告"} / 输出 ${result.usage.outputTokens ?? "未报告"} tokens`;
      for (const note of result.package.annotations) {
        if (
          !notes.value.some(
            (n) =>
              n.note.sentenceId === note.sentenceId &&
              n.note.start === note.start &&
              n.note.end === note.end &&
              n.note.kind === note.kind,
          )
        )
          notes.value.push({ note, selected: true });
      }
      done.value++;
    }
    status.value =
      done.value === jobs.value.length
        ? `解析完成，得到 ${notes.value.length} 条解读。${skipped.value ? `另有 ${skipped.value} 条未通过校验，详见批次记录。` : ""}请选择需要保存的内容。`
        : "已暂停，已完成的结果保留在下方。";
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    error.value = jobs.value.length
      ? `第 ${done.value + 1} / ${jobs.value.length} 批失败：${message}。已完成 ${done.value} 批，已有预览保留；继续将重试本批。`
      : message;
  } finally {
    busy.value = false;
  }
}
async function save() {
  if (busy.value || saving.value || !chosen.value.length) return;
  saving.value = true;
  error.value = "";
  try {
    const result = await api.importAnnotations({
      schemaVersion: 1,
      documentId: props.document.id,
      version: props.document.version,
      annotations: chosen.value.map((n) => n.note),
    });
    notes.value = notes.value.filter((n) => !n.selected);
    status.value = `已新增 ${result.added} 条解读，重复项自动跳过；原有笔记保留。`;
    emit("saved");
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}
function context(note: Annotation) {
  return props.document.chapters
    .flatMap((c) => c.sentences)
    .find((s) => s.id === note.sentenceId)?.text;
}
onMounted(async () => {
  dialog.value?.showModal();
  try {
    const c = await api.settings();
    configured.value = !!c.llm.endpoint && !!c.llm.model;
    model.value = c.llm.model;
    outputLimit.value = c.llm.maxTokens;
    timeoutSeconds.value = c.llm.timeoutSeconds;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    checking.value = false;
  }
});
</script>
<template>
  <dialog
    ref="dialog"
    class="analysis-dialog"
    aria-labelledby="analysis-title"
    @cancel.prevent="!busy && !saving && emit('close')"
  >
    <div class="dialog-heading">
      <div>
        <span class="eyebrow">READ, THEN UNDERSTAND</span>
        <h2 id="analysis-title">让词句有迹可循</h2>
      </div>
      <button
        class="close-button"
        aria-label="关闭解析预览"
        :disabled="busy || saving"
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
    <template v-if="configured">
      <fieldset
        class="chapter-picker"
        :disabled="busy || saving || jobs.length > 0"
      >
        <legend>选择章节</legend>
        <label v-for="c in document.chapters" :key="c.id"
          ><input type="checkbox" v-model="selected" :value="c.id" />{{ c.title
          }}<small>{{ c.sentences.length }} 句</small></label
        >
      </fieldset>
      <p class="muted preference-note">
        所选正文约
        {{ characters.toLocaleString() }}
        字，将分批发送到你的模型服务，可能产生费用。结果先预览，保存后才进入书库与知识手帐。
      </p>
      <label class="field-label"
        >每批最多句数
        <select
          v-model.number="batchSize"
          :disabled="busy || saving || jobs.length > 0"
        >
          <option v-for="n in [1, 2, 4, 8]" :key="n" :value="n">
            {{ n }} 句
          </option>
        </select>
      </label>
      <p class="muted preference-note">
        每批独立请求：输出上限 {{ outputLimit }} tokens，超时
        {{ timeoutSeconds }} 秒，正文最多 4000 字。默认每批 2
        句，适合先验证本地模型；各批不携带前批对话，服务商的账户／速率限制仍然适用。
      </p>
      <p v-if="lastUsage" class="muted" role="status">{{ lastUsage }}</p>
      <div class="settings-actions">
        <button
          v-if="!jobs.length || done < jobs.length"
          class="button primary"
          :disabled="busy || saving || !selected.length"
          @click="run"
        >
          {{ jobs.length ? "继续未完成的解析" : "开始解析" }}</button
        ><button
          v-if="busy"
          class="button secondary"
          :disabled="stopping"
          @click="stopping = true"
        >
          {{ stopping ? "本批完成后暂停…" : "本批完成后暂停" }}</button
        ><span v-if="jobs.length" role="status" class="muted"
          >已完成 {{ done }} / {{ jobs.length }} 批<span v-if="busy">
            · 第 {{ done + 1 }} 批等待响应 {{ elapsedSeconds }} 秒</span
          ></span
        >
      </div>
      <progress
        v-if="jobs.length"
        :value="done"
        :max="jobs.length"
        aria-label="解析批次进度"
      ></progress>
      <details
        v-if="batchResults.length"
        class="batch-history"
        :open="skipped > 0"
      >
        <summary>
          已完成批次（{{ batchResults.length }}）<span v-if="skipped">
            · 跳过 {{ skipped }} 条无效解读</span
          >
        </summary>
        <ul>
          <li v-for="result in batchResults" :key="result.batch">
            第 {{ result.batch }} 批 · {{ result.seconds }} 秒 ·
            {{
              result.count
                ? `${result.count} 条解读`
                : "模型返回空列表，没有新增解读"
            }}
            <ul v-if="result.warnings.length" class="muted">
              <li v-for="(warning, index) in result.warnings" :key="index">
                {{ warning }}
              </li>
            </ul>
          </li>
        </ul>
      </details>
    </template>
    <p
      ref="errorMessage"
      v-if="error"
      role="alert"
      class="form-error preference-message"
    >
      {{ error }}
    </p>
    <p v-if="status" role="status" class="preference-message">{{ status }}</p>
    <template v-if="notes.length"
      ><div class="analysis-review-heading">
        <h3>
          待保存的解读 <small>{{ notes.length }}</small>
        </h3>
        <button
          class="text-button"
          :disabled="busy || saving"
          @click="notes.forEach((n) => (n.selected = true))"
        >
          全选</button
        ><button
          class="text-button"
          :disabled="busy || saving"
          @click="notes.forEach((n) => (n.selected = false))"
        >
          取消全选
        </button>
      </div>
      <div class="analysis-results">
        <label v-for="(item, i) in notes" :key="i" class="analysis-result"
          ><input
            type="checkbox"
            v-model="item.selected"
            :disabled="busy || saving"
          />
          <div>
            <div class="analysis-quote">
              <strong lang="ja">{{ item.note.quote }}</strong
              ><span>{{ item.note.reading }}</span
              ><small>{{ item.note.kind }}</small>
            </div>
            <p>{{ item.note.meaning }}</p>
            <p class="muted">{{ item.note.explanation }}</p>
            <blockquote lang="ja">{{ context(item.note) }}</blockquote>
          </div></label
        >
      </div></template
    >
    <div class="analysis-footer">
      <span class="muted">{{
        notes.length ? "未保存的预览会在关闭后丢弃。" : "已有的手工解读会保留。"
      }}</span
      ><button
        v-if="notes.length"
        class="button primary"
        :disabled="busy || saving || !chosen.length"
        @click="save"
      >
        保存所选 {{ chosen.length }} 条</button
      ><button
        class="button secondary"
        :disabled="busy || saving"
        @click="emit('close')"
      >
        关闭
      </button>
    </div>
  </dialog>
</template>
