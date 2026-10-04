<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  reactive,
  ref,
  watch,
} from "vue";
import { api } from "./api";
import SettingsPage from "./SettingsPage.vue";
import {
  dark,
  fontSize,
  toggleTheme as setTheme,
  animatePage,
} from "./preferences";
import type {
  Annotation,
  Document,
  ParsePackage,
  Progress,
  Snapshot,
} from "./types";
import SentenceView from "./SentenceView.vue";
import AozoraImport from "./AozoraImport.vue";
const importMode = ref("aozora"),
  sourceBusy = ref(false);
async function sourceImported() {
  await refresh();
}
async function openSource(document: Document) {
  await refresh();
  importDialog.value?.close();
  openDocument(document);
}

const data = reactive<Snapshot>({
  documents: [],
  annotations: [],
  progress: [],
});
const page = ref("home"),
  search = ref(""),
  filter = ref("全部"),
  docId = ref(""),
  chapterId = ref(""),
  selectedId = ref("");
const pageNames: Record<string, string> = {
  home: "今日",
  library: "我的书库",
  reader: "阅读",
  knowledge: "知识手帐",
  settings: "设置",
};
watch(page, () => animatePage(document.getElementById("main")), {
  flush: "post",
});
const loading = ref(true),
  busy = ref(false),
  error = ref(""),
  notice = ref("");
const ruby = ref(true),
  directoryOpen = ref(false),
  explanationOpen = ref(false);
function closeDrawers(event: KeyboardEvent) {
  if (event.key !== "Escape" || document.querySelector("dialog[open]")) return;
  const side = document.activeElement?.closest(".reader-drawer");
  directoryOpen.value = false;
  explanationOpen.value = false;
  side?.querySelector<HTMLButtonElement>(".drawer-handle")?.focus();
}
const importDialog = ref<HTMLDialogElement>(),
  editDialog = ref<HTMLDialogElement>(),
  jsonDialog = ref<HTMLDialogElement>();
const input = reactive({
  title: "",
  author: "",
  kind: "文章",
  url: "",
  text: "",
});
const draft = ref<Annotation>(),
  jsonText = ref(""),
  jsonPreview = ref<ParsePackage>(),
  jsonExport = ref(false);
const selectionBusy = ref(false),
  selectionError = ref("");
const manualNote = ref(false);
const selectionPreview = ref<Annotation>();
const selectionPending = ref(false);
let selectionGeneration = 0;
function discardSelection() {
  selectionGeneration++;
  selectionPreview.value = undefined;
  selectionError.value = "";
  selectionPending.value = false;
}
function readerBlankClicked(event: MouseEvent) {
  if (page.value !== "reader" || !(event.target instanceof Element)) return;
  if (
    event.target.closest(
      "button,[role=button],a,input,select,textarea,dialog,.reader-drawer,summary",
    )
  )
    return;
  if (!window.getSelection()?.isCollapsed) return;
  discardSelection();
  selectedId.value = "";
  explanationOpen.value = false;
}
function chooseAnnotation(a: Annotation) {
  discardSelection();
  selectedId.value = a.id;
  explanationOpen.value = true;
  nextTick(() =>
    document.querySelector(".drawer-scroll")?.scrollTo({ top: 0 }),
  );
}
const explanationScrolling = ref(false);
let drawerScrollTimer: ReturnType<typeof setTimeout>;
function drawerScrolled() {
  explanationScrolling.value = true;
  clearTimeout(drawerScrollTimer);
  drawerScrollTimer = setTimeout(
    () => (explanationScrolling.value = false),
    900,
  );
}
watch([docId, chapterId], () => {
  discardSelection();
});
const current = computed(() =>
  data.documents.find((d) => d.id === docId.value),
);
const chapter = computed(() =>
  current.value?.chapters.find((c) => c.id === chapterId.value),
);
const selected = computed(() =>
  data.annotations.find((a) => a.id === selectedId.value),
);
const chapterNotes = computed(() =>
  data.annotations.filter((a) =>
    chapter.value?.sentences.some((s) => s.id === a.sentenceId),
  ),
);
const paragraphs = computed(() => {
  const groups: {
    paragraph: number;
    sentences: NonNullable<typeof chapter.value>["sentences"];
  }[] = [];
  for (const s of chapter.value?.sentences || []) {
    let last = groups.at(-1);
    if (!last || last.paragraph !== s.paragraph) {
      last = { paragraph: s.paragraph, sentences: [] };
      groups.push(last);
    }
    last.sentences.push(s);
  }
  return groups;
});
const filtered = computed(() =>
  data.documents.filter(
    (d) =>
      (filter.value === "全部" || d.kind === filter.value) &&
      `${d.title} ${d.author}`
        .toLowerCase()
        .includes(search.value.toLowerCase()),
  ),
);
const knowledge = computed(() =>
  data.annotations.filter((a) =>
    `${a.quote} ${a.meaning} ${a.reading}`.includes(search.value),
  ),
);
const learning = computed(() => data.annotations.filter((a) => a.learn).length);
const lastDocument = computed(
  () =>
    data.documents.find((d) => d.id === data.progress[0]?.documentId) ||
    data.documents[0],
);
const currentContext = computed(
  () =>
    chapter.value?.sentences.find((s) => s.id === selected.value?.sentenceId)
      ?.text,
);
let toastTimer: ReturnType<typeof setTimeout>,
  scrollTimer: ReturnType<typeof setTimeout>;
let restoring = false,
  progressQueue = Promise.resolve();
function message(text: string) {
  notice.value = text;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (notice.value = ""), 3500);
}
function fail(e: unknown) {
  error.value = e instanceof Error ? e.message : String(e);
}
async function action(fn: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await fn();
  } catch (e) {
    fail(e);
  } finally {
    busy.value = false;
  }
}
async function refresh() {
  Object.assign(data, await api.snapshot());
}
function navigate(to: string) {
  discardSelection();
  saveVisiblePosition();
  page.value = to;
  search.value = "";
  window.scrollTo(0, 0);
}
function remember(p: Progress) {
  const old = data.progress.find((x) => x.documentId === p.documentId);
  if (old?.chapterId === p.chapterId && old.sentenceId === p.sentenceId) return;
  data.progress = [
    p,
    ...data.progress.filter((x) => x.documentId !== p.documentId),
  ];
  progressQueue = progressQueue
    .then(async () => {
      await api.saveProgress(p);
    })
    .catch(fail);
}
function saveVisiblePosition() {
  if (restoring || page.value !== "reader" || !current.value || !chapter.value)
    return;
  const elements = [
    ...document.querySelectorAll<HTMLElement>("[data-sentence]"),
  ];
  const visible = elements.find(
    (el) => el.getBoundingClientRect().bottom > 110,
  );
  if (visible?.dataset.sentence)
    remember({
      documentId: current.value.id,
      chapterId: chapter.value.id,
      sentenceId: visible.dataset.sentence,
    });
}
function onScroll() {
  clearTimeout(scrollTimer);
  scrollTimer = setTimeout(saveVisiblePosition, 250);
}
async function openDocument(d: Document, target?: Annotation) {
  discardSelection();
  saveVisiblePosition();
  restoring = true;
  docId.value = d.id;
  page.value = "reader";
  selectedId.value = target?.id || "";
  if (target) explanationOpen.value = true;
  const saved = data.progress.find((p) => p.documentId === d.id);
  chapterId.value =
    (target
      ? d.chapters.find((c) =>
          c.sentences.some((s) => s.id === target.sentenceId),
        )?.id
      : saved?.chapterId) || d.chapters[0].id;
  await nextTick();
  const sentenceId =
    target?.sentenceId || saved?.sentenceId || chapter.value!.sentences[0].id;
  const el = document.getElementById("sentence-" + sentenceId);
  el?.scrollIntoView({ block: "center" });
  if (target) {
    el?.classList.add("sentence-flash");
    setTimeout(() => el?.classList.remove("sentence-flash"), 1800);
  }
  remember({ documentId: d.id, chapterId: chapterId.value, sentenceId });
  setTimeout(() => (restoring = false), 350);
}
async function switchChapter(id: string) {
  saveVisiblePosition();
  restoring = true;
  chapterId.value = id;
  selectedId.value = "";
  await nextTick();
  window.scrollTo(0, 0);
  remember({
    documentId: docId.value,
    chapterId: id,
    sentenceId: chapter.value!.sentences[0].id,
  });
  setTimeout(() => (restoring = false), 350);
}
async function importText() {
  await action(async () => {
    const chunks =
      input.kind === "小说" ? input.text.split(/^---\s*$/m) : [input.text];
    const d = await api.importDocument({
      ...input,
      chapters: chunks.map((text, i) => ({
        title: input.kind === "小说" ? `第 ${i + 1} 章` : "正文",
        text,
      })),
    });
    await refresh();
    importDialog.value?.close();
    Object.assign(input, {
      title: "",
      author: "",
      kind: "文章",
      url: "",
      text: "",
    });
    await openDocument(d);
    message("正文已保存，可以开始阅读");
  });
}
function textWithoutRuby(fragment: DocumentFragment) {
  fragment.querySelectorAll("rt,rp").forEach((el) => el.remove());
  return fragment.textContent || "";
}
function captureSelection(): Annotation | undefined {
  const selection = window.getSelection();
  if (!selection?.rangeCount || selection.isCollapsed) {
    message("请先在同一句内划选需要解释的文字");
    return;
  }
  const range = selection.getRangeAt(0);
  const parent = (node: Node) =>
    (node.nodeType === Node.ELEMENT_NODE
      ? (node as Element)
      : node.parentElement
    )?.closest<HTMLElement>("[data-sentence]");
  const startEl = parent(range.startContainer),
    endEl = parent(range.endContainer);
  if (!startEl || startEl !== endEl) {
    message("本版本请在同一句内选择；跨句解释留待后续支持");
    return;
  }
  if (
    range.startContainer.parentElement?.closest("rt") ||
    range.endContainer.parentElement?.closest("rt")
  ) {
    message("请选择正文文字，不要从注音开始或结束");
    return;
  }
  const before = range.cloneRange();
  before.selectNodeContents(startEl);
  before.setEnd(range.startContainer, range.startOffset);
  const start = Array.from(textWithoutRuby(before.cloneContents())).length;
  const quote = textWithoutRuby(range.cloneContents());
  const end = start + Array.from(quote).length;
  if (!quote.trim() || !current.value) return;
  return {
    id: "",
    documentId: current.value.id,
    version: current.value.version,
    sentenceId: startEl.dataset.sentence!,
    start,
    end,
    quote,
    kind: "词汇",
    reading: "",
    meaning: "",
    explanation: "",
    learn: true,
  };
}
async function explainSelection() {
  if (selectionBusy.value) return;
  const target = captureSelection();
  if (!target) return;
  window.getSelection()?.removeAllRanges();
  const targetChapter = chapterId.value;
  const generation = ++selectionGeneration;
  selectedId.value = "";
  selectionBusy.value = true;
  selectionPending.value = true;
  selectionError.value = "";
  selectionPreview.value = undefined;
  explanationOpen.value = true;
  await nextTick();
  document.querySelector(".drawer-scroll")?.scrollTo({ top: 0 });
  try {
    const result = await api.explainSelection(
      target.documentId,
      targetChapter,
      target,
    );
    if (
      generation === selectionGeneration &&
      docId.value === target.documentId &&
      chapterId.value === targetChapter
    )
      selectionPreview.value = result.package.annotations[0];
  } catch (e) {
    if (
      generation === selectionGeneration &&
      docId.value === target.documentId &&
      chapterId.value === targetChapter
    )
      selectionError.value = e instanceof Error ? e.message : String(e);
  } finally {
    selectionBusy.value = false;
    selectionPending.value = false;
  }
}
function addManualNote() {
  const hasSelection = !window.getSelection()?.isCollapsed;
  const base = hasSelection
    ? captureSelection()
    : selected.value || selectionPreview.value;
  if (!base) {
    message("先划选正文，或点击一条词句解读，再添加注释");
    return;
  }
  draft.value = {
    ...base,
    id: "",
    meaning: "",
    explanation: "",
    reading: "",
    learn: false,
  };
  manualNote.value = true;
  editDialog.value?.showModal();
}
function edit(a: Annotation) {
  manualNote.value = false;
  draft.value = { ...a };
  editDialog.value?.showModal();
}
async function saveNote() {
  await action(async () => {
    const a = await api.saveAnnotation(draft.value!);
    await refresh();
    selectedId.value = a.id;
    selectionPreview.value = undefined;
    explanationOpen.value = true;
    editDialog.value?.close();
    message("解释已保存");
  });
}
async function toggleLearning(a: Annotation) {
  await action(async () => {
    const saved = await api.setLearning(a.id, !a.learn);
    Object.assign(a, saved);
  });
}
function showJson(exporting: boolean) {
  jsonExport.value = exporting;
  jsonPreview.value = undefined;
  if (exporting && current.value) {
    const d = current.value;
    jsonText.value = JSON.stringify(
      {
        instructions:
          "请返回 responseTemplate 形状的 JSON；只返回该对象，不含 instructions/source。start/end 从 0 起，按 Unicode 字符计数，end 不包含末尾。quote 必须与原文范围一致。kind 为词汇/语法/读音，id 留空，learn 为 true。每条填写非空 meaning。",
        source: d,
        responseTemplate: {
          schemaVersion: 1,
          documentId: d.id,
          version: d.version,
          annotations: [],
        },
        annotationExample: {
          id: "",
          documentId: d.id,
          version: d.version,
          sentenceId: d.chapters[0].sentences[0].id,
          start: 0,
          end: 1,
          quote: Array.from(d.chapters[0].sentences[0].text)[0],
          kind: "词汇",
          reading: "",
          meaning: "请填写语境释义",
          explanation: "",
          learn: true,
        },
      },
      null,
      2,
    );
  } else jsonText.value = "";
  jsonDialog.value?.showModal();
}
function previewJson() {
  try {
    const p = JSON.parse(jsonText.value) as ParsePackage;
    if (
      p.schemaVersion !== 1 ||
      p.documentId !== current.value?.id ||
      p.version !== current.value.version ||
      !Array.isArray(p.annotations) ||
      !p.annotations.length
    )
      throw new Error("需要当前作品、当前版本的解析包和至少一条标注");
    const sentences = current.value.chapters.flatMap((c) => c.sentences);
    for (const a of p.annotations) {
      const s = sentences.find((s) => s.id === a.sentenceId);
      if (
        a.documentId !== p.documentId ||
        a.version !== p.version ||
        !s ||
        !Number.isInteger(a.start) ||
        !Number.isInteger(a.end) ||
        a.start < 0 ||
        a.start >= a.end ||
        a.end > Array.from(s.text).length ||
        Array.from(s.text).slice(a.start, a.end).join("") !== a.quote ||
        typeof a.meaning !== "string" ||
        !a.meaning.trim() ||
        !["词汇", "语法", "读音"].includes(a.kind)
      )
        throw new Error("存在无效标注，请检查句子 ID、字符范围、引用和释义");
    }
    jsonPreview.value = p;
    error.value = "";
  } catch (e) {
    jsonPreview.value = undefined;
    fail(e);
  }
}
async function importJson() {
  await action(async () => {
    const result = await api.importAnnotations(jsonPreview.value!);
    await refresh();
    jsonDialog.value?.close();
    message(
      `已新增 ${result.added} 条；已有同位置同类型标注保留原解释和学习选择`,
    );
  });
}
async function copyJson() {
  try {
    await navigator.clipboard.writeText(jsonText.value);
    message("已复制 JSON");
  } catch {
    message("无法访问剪贴板，请在文本框内全选复制");
  }
}
onMounted(async () => {
  try {
    await refresh();
  } catch (e) {
    fail(e);
  } finally {
    loading.value = false;
  }
  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("keydown", closeDrawers);
});
onUnmounted(() => {
  window.removeEventListener("scroll", onScroll);
  window.removeEventListener("keydown", closeDrawers);
  clearTimeout(scrollTimer);
  clearTimeout(toastTimer);
  clearTimeout(drawerScrollTimer);
});
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="navigate('home')"
        ><span class="brand-symbol">読<span>。</span></span
        ><span>JapRead<small>日语阅读与学习</small></span></a
      >
      <div class="workspace-label">我的学习空间 <span>LOCAL</span></div>
      <nav aria-label="主导航">
        <button
          class="nav-item"
          :class="{ active: page === 'home' }"
          @click="navigate('home')"
        >
          <span class="nav-icon">⌂</span>今日
        </button>
        <button
          class="nav-item"
          :class="{ active: ['library', 'reader'].includes(page) }"
          @click="navigate('library')"
        >
          <span class="nav-icon">▤</span>我的书库<span class="nav-count">{{
            data.documents.length
          }}</span>
        </button>
        <div class="nav-divider"></div>
        <button
          class="nav-item"
          :class="{ active: page === 'knowledge' }"
          @click="navigate('knowledge')"
        >
          <span class="nav-icon">❧</span>知识手帐<span class="nav-count">{{
            data.annotations.length
          }}</span>
        </button>
      </nav>
      <div class="sidebar-note">
        <span>一日一歩。</span>
        <p>不急着读完，<br />让理解慢慢发生。</p>
        <div class="note-lines"></div>
      </div>
      <div class="sidebar-bottom">
        <button
          class="nav-item"
          :class="{ active: page === 'settings' }"
          @click="navigate('settings')"
        >
          <span class="nav-icon">⚙</span> 设置
        </button>
      </div>
    </aside>
    <div class="main-shell">
      <header class="topbar">
        <div class="breadcrumb">
          学习空间 <span>/</span><b>{{ pageNames[page] }}</b>
        </div>
        <div class="topbar-actions">
          <button class="theme-toggle" :aria-pressed="dark" @click="setTheme">
            {{ dark ? "☀ 浅色" : "☾ 暗夜" }}</button
          ><button
            class="import-button"
            :disabled="loading"
            @click="importDialog?.showModal()"
          >
            ＋ 导入内容
          </button>
        </div>
      </header>
      <div v-if="error" class="error-banner" role="alert">
        {{ error
        }}<button @click="error = ''" aria-label="关闭错误提示">×</button>
      </div>
      <main id="main" @click="readerBlankClicked">
        <div v-if="loading" class="empty-state">正在打开你的学习空间…</div>
        <template v-else-if="page === 'home'">
          <div class="page-heading">
            <div>
              <span class="eyebrow">A LITTLE READING, EVERY DAY</span>
              <h1>在阅读中，与日语相遇。</h1>
              <p class="subtitle">读懂一个句子，也是在向前走一步。</p>
            </div>
            <span class="date-label">{{
              new Date().toLocaleDateString("zh-CN", {
                month: "long",
                day: "numeric",
              })
            }}</span>
          </div>
          <section class="home-hero">
            <div>
              <span class="eyebrow">{{
                lastDocument ? "CONTINUE YOUR STORY" : "YOUR FIRST PAGE"
              }}</span>
              <h2>{{ lastDocument?.title || "从你喜欢的一篇日文开始" }}</h2>
              <p>
                {{
                  lastDocument
                    ? `${lastDocument.author || "我的收藏"} · ${lastDocument.kind} · ${lastDocument.chapters.length} 个章节`
                    : "新闻、随笔，或小说的一章。把原文带进来，把理解留在这里。"
                }}
              </p>
              <button
                class="button primary"
                @click="
                  lastDocument
                    ? openDocument(lastDocument)
                    : importDialog?.showModal()
                "
              >
                {{ lastDocument ? "继续阅读 →" : "导入第一篇 →" }}
              </button>
            </div>
            <div class="hero-book" aria-hidden="true">
              <span>日本語の時間</span><strong>読<br />む。</strong
              ><small>一頁、一歩。</small>
              <div class="book-branch">❧</div>
            </div>
          </section>
          <div class="reading-stats">
            <div>
              <strong>{{ data.documents.length }}</strong
              ><span>篇作品，等你翻阅</span>
            </div>
            <div>
              <strong>{{ data.annotations.length }}</strong
              ><span>条语境，留下理解</span>
            </div>
            <div>
              <strong>{{ learning }}</strong
              ><span>个知识点，加入学习</span>
            </div>
          </div>
          <div class="section-heading">
            <h2>书架上的时光</h2>
            <button class="text-button" @click="navigate('library')">
              查看书库 →
            </button>
          </div>
          <div v-if="!data.documents.length" class="empty-state">
            <p>书架还空着。导入后，正文和笔记会保存在本机。</p>
          </div>
          <div class="real-book-grid">
            <button
              v-for="d in data.documents.slice(0, 3)"
              :key="d.id"
              class="real-book-card"
              @click="openDocument(d)"
            >
              <span class="tag">{{ d.kind }}</span>
              <h3>{{ d.title }}</h3>
              <p>{{ d.author || "我的收藏" }}</p>
              <div class="book-excerpt">
                {{
                  d.chapters[0].sentences
                    .slice(0, 2)
                    .map((s) => s.text)
                    .join("")
                }}
              </div>
              <span class="text-button">翻开阅读 ↗</span>
            </button>
          </div>
        </template>
        <template v-else-if="page === 'library'">
          <div class="page-heading">
            <div>
              <span class="eyebrow">YOUR READING SHELF</span>
              <h1>我的书库</h1>
              <p class="subtitle">喜欢的文字，值得慢慢读。</p>
            </div>
          </div>
          <div class="library-toolbar">
            <div class="filter-tabs">
              <button
                v-for="kind in ['全部', '新闻', '文章', '小说']"
                :key="kind"
                :class="{ active: filter === kind }"
                @click="filter = kind"
              >
                {{ kind }}
              </button>
            </div>
            <input
              v-model="search"
              aria-label="搜索书库"
              placeholder="搜索标题或作者…"
            />
          </div>
          <div class="real-book-grid">
            <button
              v-for="d in filtered"
              :key="d.id"
              class="real-book-card"
              @click="openDocument(d)"
            >
              <span class="tag">{{ d.kind }}</span>
              <h3>{{ d.title }}</h3>
              <p>{{ d.author || "我的收藏" }} · {{ d.chapters.length }} 章</p>
              <div class="book-excerpt">
                {{
                  d.chapters[0].sentences
                    .slice(0, 2)
                    .map((s) => s.text)
                    .join("")
                }}
              </div>
              <span class="text-button"
                >{{
                  data.annotations.some((a) => a.documentId === d.id)
                    ? "已有解读"
                    : "待添加解读"
                }}
                · 阅读 →</span
              >
            </button>
          </div>
          <div v-if="!filtered.length" class="empty-state">
            {{
              data.documents.length
                ? "没有找到匹配的作品"
                : "导入第一篇正文，建立自己的书库。"
            }}
          </div>
        </template>
        <template v-else-if="page === 'reader' && current && chapter">
          <div class="reader-heading">
            <button class="text-button" @click="navigate('library')">
              ← 我的书库 / {{ current.title }}
            </button>
            <div class="reader-tools">
              <button
                class="button secondary"
                :aria-pressed="ruby"
                @click="ruby = !ruby"
              >
                あ {{ ruby ? "隐藏注音" : "显示注音" }}
              </button>
            </div>
          </div>
          <div class="reader-layout reader-with-drawers">
            <div
              class="reader-drawer drawer-left"
              :class="{ 'is-open': directoryOpen }"
            >
              <button
                class="drawer-handle"
                :aria-expanded="directoryOpen"
                aria-controls="reading-directory"
                :aria-label="
                  directoryOpen ? '收起目录与字号' : '展开目录与字号'
                "
                :title="directoryOpen ? '收起目录与字号' : '展开目录与字号'"
                @click="directoryOpen = !directoryOpen"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    :d="directoryOpen ? 'm14 6-6 6 6 6' : 'm10 6 6 6-6 6'"
                  /></svg
                ><span>目录</span>
              </button>
              <aside
                id="reading-directory"
                class="chapter-nav drawer-panel"
                :inert="!directoryOpen"
                :aria-hidden="!directoryOpen"
                aria-label="目录与阅读设置"
              >
                <div class="drawer-title">
                  <h2>目录与阅读</h2>
                  <span>CONTENTS</span>
                </div>
                <p>阅读目录 · {{ current.chapters.length }} 章</p>
                <button
                  v-for="(c, i) in current.chapters"
                  :key="c.id"
                  class="chapter-button"
                  :class="{ active: c.id === chapterId }"
                  @click="switchChapter(c.id)"
                >
                  <span>{{ String(i + 1).padStart(2, "0") }}</span
                  >{{ c.title }}
                </button>
                <section class="font-settings" aria-label="阅读字号设置">
                  <label for="reading-font-size"
                    >正文字号
                    <output for="reading-font-size"
                      >{{ fontSize }} px</output
                    ></label
                  >
                  <input
                    id="reading-font-size"
                    v-model.number="fontSize"
                    type="range"
                    min="16"
                    max="32"
                    step="1"
                    aria-label="正文字号"
                  />
                  <div class="font-actions">
                    <button
                      class="button secondary"
                      :disabled="fontSize <= 16"
                      aria-label="减小字号"
                      @click="fontSize = Math.max(16, fontSize - 1)"
                    >
                      A−</button
                    ><button class="text-button" @click="fontSize = 20">
                      恢复默认</button
                    ><button
                      class="button secondary"
                      :disabled="fontSize >= 32"
                      aria-label="增大字号"
                      @click="fontSize = Math.min(32, fontSize + 1)"
                    >
                      A＋
                    </button>
                  </div>
                </section>
                <div class="chapter-actions">
                  <button class="text-button" @click="showJson(true)">
                    导出解析模板 ↗</button
                  ><button class="text-button" @click="showJson(false)">
                    导入解析 JSON ＋
                  </button>
                </div>
              </aside>
            </div>
            <article class="reading-paper" :key="chapter.id">
              <span class="eyebrow">{{
                current.kind === "小说"
                  ? "A CHAPTER OF YOUR STORY"
                  : "WORDS IN CONTEXT"
              }}</span>
              <h1>{{ current.title }}</h1>
              <p class="byline">
                {{ current.author || "我的收藏" }} · {{ chapter.title }}
                <a
                  v-if="current.url"
                  :href="current.url"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  · 原文 ↗</a
                >
              </p>
              <div
                class="japanese-body"
                lang="ja"
                :style="{ fontSize: fontSize + 'px' }"
              >
                <p v-for="group in paragraphs" :key="group.paragraph">
                  <SentenceView
                    v-for="s in group.sentences"
                    :key="s.id"
                    :sentence="s"
                    :annotations="
                      chapterNotes.filter((a) => a.sentenceId === s.id)
                    "
                    :active="selectedId"
                    :ruby="ruby"
                    @choose="chooseAnnotation"
                  />
                </p>
              </div>
              <details v-if="current.source" class="source-provenance">
                <summary>青空文库 · 原书注音与来源说明</summary>
                <p>{{ current.source.notice }}</p>
                <p v-if="current.source.credits">
                  {{ current.source.credits }}
                </p>
              </details>
              <div class="reader-bottom">
                <span>划选同一句内的文字，点击右下角「解读选中文字」。</span>
              </div>
            </article>
            <div
              class="reader-drawer drawer-right"
              :class="{ 'is-open': explanationOpen }"
            >
              <button
                class="drawer-handle"
                :aria-expanded="explanationOpen"
                aria-controls="reading-explanation"
                :aria-label="explanationOpen ? '收起词句解读' : '展开词句解读'"
                :title="explanationOpen ? '收起词句解读' : '展开词句解读'"
                @click="explanationOpen = !explanationOpen"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    :d="explanationOpen ? 'm10 6 6 6-6 6' : 'm14 6-6 6 6 6'"
                  /></svg
                ><span>解读</span>
              </button>
              <aside
                id="reading-explanation"
                class="annotation-panel drawer-panel"
                :inert="!explanationOpen"
                :aria-hidden="!explanationOpen"
                aria-label="词句解读"
              >
                <div class="annotation-tabs">
                  <strong>词句解读</strong><span>从原文出发</span>
                </div>
                <div
                  class="drawer-scroll"
                  :class="{ 'is-scrolling': explanationScrolling }"
                  @scroll.passive="drawerScrolled"
                  tabindex="0"
                  aria-label="解读内容"
                >
                  <button
                    class="text-button add-manual-note"
                    @mousedown.prevent
                    @click="addManualNote"
                  >
                    ＋ 添加注释
                  </button>
                  <p v-if="selectionPending" role="status">正在解读所选文字…</p>
                  <p v-if="selectionError" role="alert" class="form-error">
                    {{ selectionError }}
                  </p>
                  <section v-if="selectionPreview" class="annotation-card">
                    <span class="tag">LLM 划词解读 · 未保存</span>
                    <div class="word-title">
                      <h2>{{ selectionPreview.quote }}</h2>
                    </div>
                    <p class="pronunciation">{{ selectionPreview.reading }}</p>
                    <p class="definition">{{ selectionPreview.meaning }}</p>
                    <p class="definition">{{ selectionPreview.explanation }}</p>
                    <button class="text-button" @click="edit(selectionPreview)">
                      检查并保存解读 ↗
                    </button>
                    <button class="text-button" @click="discardSelection">
                      丢弃
                    </button>
                  </section>
                  <section v-if="selected" class="annotation-card">
                    <span class="tag">{{ selected.kind }}</span>
                    <div class="word-title">
                      <h2>{{ selected.quote }}</h2>
                    </div>
                    <p class="pronunciation">{{ selected.reading }}</p>
                    <p class="definition">{{ selected.meaning }}</p>
                    <div class="context-label">在这句话里</div>
                    <div class="context-box" lang="ja">
                      {{ currentContext }}
                    </div>
                    <p class="definition">{{ selected.explanation }}</p>
                    <label class="learn-checkbox"
                      ><input
                        type="checkbox"
                        :checked="selected.learn"
                        :disabled="busy"
                        @change="toggleLearning(selected)"
                      />加入学习 / 测验</label
                    ><button
                      class="text-button edit-note"
                      @click="edit(selected)"
                    >
                      编辑解释 ↗
                    </button>
                  </section>
                  <section
                    v-else-if="!selectionPreview && !selectionPending"
                    class="annotation-card"
                  >
                    <span class="tag">慢慢读，慢慢懂</span>
                    <p class="definition">
                      点击标记查看解释，或划选正文让 LLM
                      解读。自己的笔记可通过「添加注释」保存。
                    </p>
                    <p class="reader-help">
                      没有 API 也能使用。助手生成的解析可通过 JSON 导入。
                    </p>
                  </section>
                  <section
                    v-if="!selected && !selectionPreview && !selectionPending"
                    class="mini-card"
                  >
                    <span class="eyebrow">IN THIS CHAPTER</span
                    ><strong>{{ chapterNotes.length }} 条语境笔记</strong
                    ><button
                      v-for="a in chapterNotes"
                      :key="a.id"
                      class="note-link"
                      @click="chooseAnnotation(a)"
                    >
                      {{ a.quote }}<small>{{ a.kind }}</small>
                    </button>
                  </section>
                </div>
              </aside>
            </div>
          </div>
          <button
            class="selection-action button primary"
            :class="{ 'with-explanation': explanationOpen }"
            @mousedown.prevent
            :disabled="selectionBusy"
            @click="explainSelection"
          >
            {{ selectionBusy ? "正在解读…" : "解读选中文字 · LLM" }}
          </button>
        </template>
        <template v-else-if="page === 'knowledge'">
          <div class="page-heading">
            <div>
              <span class="eyebrow">COLLECT UNDERSTANDING</span>
              <h1>知识手帐</h1>
              <p class="subtitle">每一个知识点，都有一段相遇的原文。</p>
            </div>
            <span class="tag">{{ learning }} 个加入学习</span>
          </div>
          <input
            class="knowledge-search"
            v-model="search"
            aria-label="搜索知识"
            placeholder="搜索词句、读音或释义…"
          />
          <div v-if="!knowledge.length" class="empty-state">
            在阅读中添加解读，它们就会出现在这里。
          </div>
          <table v-else class="knowledge-table">
            <thead>
              <tr>
                <th>学习</th>
                <th>词句 / 读音</th>
                <th>语境释义</th>
                <th>来源</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="a in knowledge" :key="a.id">
                <td>
                  <input
                    type="checkbox"
                    :aria-label="'学习 ' + a.quote"
                    :checked="a.learn"
                    :disabled="busy"
                    @change="toggleLearning(a)"
                  />
                </td>
                <td>
                  <strong>{{ a.quote }}</strong
                  ><small>{{ a.kind }} · {{ a.reading }}</small>
                </td>
                <td>{{ a.meaning }}</td>
                <td>
                  <button
                    class="text-button"
                    @click="
                      openDocument(
                        data.documents.find((d) => d.id === a.documentId)!,
                        a,
                      )
                    "
                  >
                    {{
                      data.documents.find((d) => d.id === a.documentId)?.title
                    }}
                    ↗
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </template>
        <SettingsPage
          v-else-if="page === 'settings'"
          @updated="action(refresh)"
        />
      </main>
      <footer class="app-footer">
        <span>JapRead <b>·</b> 少一点机械记忆，多一点语境。</span
        ><span>LOCAL READING / 0.1</span>
      </footer>
    </div>
  </div>
  <dialog
    ref="importDialog"
    class="import-dialog"
    @cancel="sourceBusy && $event.preventDefault()"
  >
    <div class="dialog-heading">
      <div>
        <span class="eyebrow">YOUR NEXT READ</span>
        <h2>带一篇文章进来</h2>
      </div>
      <button
        type="button"
        class="icon-button"
        :disabled="sourceBusy"
        @click="importDialog?.close()"
        aria-label="关闭导入"
      >
        ×
      </button>
    </div>
    <div class="import-tabs" role="tablist" aria-label="导入方式">
      <button
        type="button"
        role="tab"
        :aria-selected="importMode === 'aozora'"
        :disabled="sourceBusy"
        @click="importMode = 'aozora'"
      >
        青空文库
      </button>
      <button
        type="button"
        role="tab"
        :aria-selected="importMode === 'paste'"
        :disabled="sourceBusy"
        @click="importMode = 'paste'"
      >
        粘贴正文
      </button>
      <button type="button" role="tab" disabled aria-selected="false">
        RSS <small>稍后接入</small>
      </button>
    </div>
    <AozoraImport
      v-show="importMode === 'aozora'"
      :documents="data.documents"
      @imported="sourceImported"
      @open="openSource"
      @busy="sourceBusy = $event"
    />
    <form v-show="importMode === 'paste'" @submit.prevent="importText">
      <p class="muted">原文留在本机，无需配置 API。</p>
      <label class="field-label"
        >标题<input
          v-model="input.title"
          required
          maxlength="200"
          placeholder="给这篇文字起个名字"
      /></label>
      <div class="form-row">
        <label class="field-label"
          >类型<select v-model="input.kind">
            <option>文章</option>
            <option>新闻</option>
            <option>小说</option>
          </select></label
        ><label class="field-label"
          >作者<input v-model="input.author" placeholder="可选"
        /></label>
      </div>
      <label class="field-label"
        >日文正文<textarea
          v-model="input.text"
          required
          rows="8"
          placeholder="ここに日本語の文章を貼り付けてください。"
        ></textarea>
      </label>
      <p class="muted" v-if="input.kind === '小说'">
        在单独一行输入 --- 分隔章节，导入后可逐章阅读。
      </p>
      <label class="field-label"
        >原文链接<input
          v-model="input.url"
          type="url"
          placeholder="https://（可选）"
      /></label>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-actions">
        <button
          type="button"
          class="button secondary"
          @click="importDialog?.close()"
        >
          取消</button
        ><button class="button primary" :disabled="busy">
          {{ busy ? "正在保存…" : "导入并阅读 →" }}
        </button>
      </div>
    </form>
  </dialog>
  <dialog ref="editDialog">
    <form v-if="draft" @submit.prevent="saveNote">
      <div class="dialog-heading">
        <div>
          <span class="eyebrow">WORDS IN CONTEXT</span>
          <h2>
            {{
              manualNote ? "添加注释" : draft.id ? "编辑解读" : "保存 LLM 解读"
            }}
          </h2>
        </div>
        <button
          type="button"
          class="icon-button"
          @click="editDialog?.close()"
          aria-label="关闭解读"
        >
          ×
        </button>
      </div>
      <blockquote lang="ja">{{ draft.quote }}</blockquote>
      <div class="form-row">
        <label class="field-label"
          >类型<select v-model="draft.kind">
            <option>词汇</option>
            <option>语法</option>
            <option>读音</option>
          </select></label
        ><label class="field-label"
          >读音<input v-model="draft.reading" placeholder="ふりがな（可选）"
        /></label>
      </div>
      <label class="field-label"
        >{{ manualNote ? "注释摘要" : "语境释义"
        }}<input
          v-model="draft.meaning"
          required
          placeholder="在这句话里，它的意思是…" /></label
      ><label class="field-label"
        >{{ manualNote ? "我的笔记" : "补充解释"
        }}<textarea
          v-model="draft.explanation"
          rows="4"
          placeholder="用法、语气，或你自己的理解。"
        ></textarea></label
      ><label class="learn-checkbox"
        ><input type="checkbox" v-model="draft.learn" />加入学习 / 测验</label
      >
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div class="dialog-actions">
        <button class="button primary" :disabled="busy">
          {{ busy ? "正在保存…" : manualNote ? "保存注释" : "保存解读" }}
        </button>
      </div>
    </form>
  </dialog>
  <dialog ref="jsonDialog" class="json-dialog">
    <div class="dialog-heading">
      <div>
        <span class="eyebrow">UNDERSTANDING, TOGETHER</span>
        <h2>{{ jsonExport ? "导出解析模板" : "导入解析 JSON" }}</h2>
      </div>
      <button
        class="icon-button"
        @click="jsonDialog?.close()"
        aria-label="关闭 JSON"
      >
        ×
      </button>
    </div>
    <p class="muted">
      {{
        jsonExport
          ? "复制给助手，按模板返回解析结果，再从「导入解析 JSON」保存。"
          : "粘贴助手返回的解析对象。先检查预览，再保存；重复位置保留已有解释与学习选择。"
      }}
    </p>
    <textarea
      v-model="jsonText"
      :readonly="jsonExport"
      rows="14"
      aria-label="解析 JSON"
      spellcheck="false"
      @input="jsonPreview = undefined"
    ></textarea>
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <div v-if="jsonPreview" class="json-preview">
      <p>
        通过前端位置校验 ·
        {{ jsonPreview.annotations.length }} 条（保存时再次完整校验）
      </p>
      <p v-for="(a, i) in jsonPreview.annotations.slice(0, 10)" :key="i">
        <b>{{ a.quote }}</b> · {{ a.kind }} · {{ a.meaning }}
      </p>
      <small v-if="jsonPreview.annotations.length > 10">仅预览前 10 条</small>
    </div>
    <div class="dialog-actions">
      <button v-if="jsonExport" class="button primary" @click="copyJson">
        复制完整模板</button
      ><template v-else
        ><button class="button secondary" @click="previewJson">
          检查并预览</button
        ><button
          class="button primary"
          :disabled="busy || !jsonPreview"
          @click="importJson"
        >
          确认保存
        </button></template
      >
    </div>
  </dialog>
  <div id="toast" :class="{ show: notice }" role="status">{{ notice }}</div>
</template>
