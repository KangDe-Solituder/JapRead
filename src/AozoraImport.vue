<script setup lang="ts">
import { computed, ref } from "vue";
import { api } from "./api";
import type {
  Document,
  SourceAuthor,
  SourceCatalog,
  SourceWork,
} from "./types";
const props = defineProps<{ documents: Document[] }>();
const emit = defineEmits<{
  imported: [];
  open: [document: Document];
  busy: [value: boolean];
}>();
const query = ref("");
const workQuery = ref("");
const result = ref<SourceCatalog>();
const author = ref<SourceAuthor>();
const categories = ref<string[]>([]),
  orthographies = ref<string[]>([]);
const offset = ref(0),
  loading = ref(false),
  importing = ref(false),
  cancel = ref(false),
  error = ref("");
const selected = ref<Record<string, SourceWork>>({});
const status = ref<
  Record<string, { message: string; document?: Document; failed?: boolean }>
>({});
const current = ref("");
let generation = 0;
const selectedCount = computed(() => Object.keys(selected.value).length);
const cachedDate = computed(() =>
  result.value ? new Date(result.value.cachedAt * 1000).toLocaleString() : "",
);
function existing(w: SourceWork) {
  return (
    props.documents.find((d) => d.source?.key === `aozora:${w.id}`) ||
    status.value[w.id]?.document
  );
}
async function load(refresh = false) {
  const own = ++generation;
  loading.value = true;
  error.value = "";
  try {
    const data = await api.sourceCatalog({
      query: query.value,
      authorId: author.value?.id,
      workQuery: workQuery.value,
      offset: offset.value,
      categories: result.value ? categories.value : undefined,
      orthographies: result.value ? orthographies.value : undefined,
      refresh,
    });
    if (own !== generation) return;
    if (!result.value) {
      categories.value = data.categories;
      orthographies.value = data.orthographies;
    }
    result.value = data;
  } catch (e) {
    if (own === generation)
      error.value = String(e instanceof Error ? e.message : e);
  } finally {
    if (own === generation) loading.value = false;
  }
}
async function searchAuthor() {
  author.value = undefined;
  offset.value = 0;
  await load();
}
async function chooseAuthor(value: SourceAuthor) {
  author.value = value;
  workQuery.value = "";
  offset.value = 0;
  await load();
}
function filterChanged() {
  offset.value = 0;
  void load();
}
function toggle(w: SourceWork, checked: boolean) {
  if (checked && selectedCount.value >= 10) {
    error.value = "一次最多选 10 部作品；完成后可以继续选择。";
    return;
  }
  if (checked) selected.value[w.id] = w;
  else delete selected.value[w.id];
}
function selectPage() {
  for (const w of result.value?.works || []) {
    if (selectedCount.value >= 10) break;
    if (w.htmlUrl && !existing(w)) selected.value[w.id] = w;
  }
}
async function importSelected() {
  importing.value = true;
  emit("busy", true);
  cancel.value = false;
  error.value = "";
  const works = Object.values(selected.value);
  try {
    for (const w of works) {
      if (cancel.value) break;
      current.value = w.id;
      status.value[w.id] = { message: "正在获取正文…" };
      try {
        const response = await api.sourceImport(w.id);
        status.value[w.id] = {
          message: response.existing ? "已在书库" : "导入完成",
          document: response.document,
        };
        delete selected.value[w.id];
        emit("imported");
      } catch (e) {
        status.value[w.id] = {
          message: String(e instanceof Error ? e.message : e),
          failed: true,
        };
        // Stop the batch on failure; do not probe more works while the source may be unavailable.
        error.value =
          "本批次已暂停。成功项已保存，其余仍保留勾选；可稍后重试。";
        break;
      }
    }
  } finally {
    current.value = "";
    importing.value = false;
    emit("busy", false);
  }
}
</script>
<template>
  <section class="aozora-import" aria-label="青空文库导入">
    <div v-if="!author" class="source-intro">
      <span class="source-seal" lang="ja">青</span>
      <div>
        <h3>从一位作家开始</h3>
        <p class="muted">先浏览目录，勾选后才下载正文。原书注音一同保留。</p>
      </div>
    </div>
    <form v-if="!author" class="source-search" @submit.prevent="searchAuthor">
      <input
        v-model="query"
        aria-label="作者姓名"
        placeholder="作者姓名／假名，例如 夏目、芥川、なつめ"
        :disabled="importing || loading"
      />
      <button class="button primary" :disabled="loading || importing">
        {{ loading ? "读取目录…" : "查找作者" }}
      </button>
    </form>
    <div v-if="!result && !loading" class="source-empty">
      <span class="eyebrow">A LIBRARY, AT YOUR PACE</span>
      <p>夏目漱石 · 芥川竜之介 · 宮沢賢治</p>
      <small>首次获取官方目录；随后在本机检索，目录缓存 24 小时。</small>
    </div>
    <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    <template v-if="result">
      <p v-if="result.warning" class="source-warning" role="status">
        {{ result.warning }}
      </p>
      <template v-if="!author">
        <p class="muted">
          找到 {{ result.authorTotal }} 位作者<span
            v-if="result.authorTotal > 100"
            >，显示前 100 位，请缩小检索范围</span
          >
        </p>
        <div class="source-authors">
          <button
            v-for="a in result.authors"
            :key="a.id"
            type="button"
            class="source-author"
            :disabled="loading || importing"
            @click="chooseAuthor(a)"
          >
            <strong lang="ja">{{ a.name }}</strong
            ><small lang="ja">{{ a.reading }}</small
            ><span>{{ a.count }} 部作品 →</span>
          </button>
        </div>
        <p v-if="!result.authors.length" class="source-empty">
          没有找到作者，试试姓氏或日文假名。
        </p>
      </template>
      <template v-else>
        <div class="source-author-heading">
          <div>
            <small class="eyebrow">AUTHOR'S SHELF</small>
            <h3 lang="ja">
              {{ author.name }} <span>{{ result.workTotal }} 部</span>
            </h3>
          </div>
          <button
            type="button"
            class="text-link"
            :disabled="importing || loading"
            @click="author = undefined"
          >
            更换作者 ←
          </button>
        </div>
        <form
          class="source-search source-work-search"
          @submit.prevent="filterChanged"
        >
          <input
            v-model="workQuery"
            aria-label="作品名"
            placeholder="在这位作者的作品中查找…"
            :disabled="importing || loading"
          />
          <button
            type="submit"
            class="button secondary"
            :disabled="importing || loading"
          >
            筛选作品
          </button>
        </form>
        <fieldset class="source-filters" :disabled="importing || loading">
          <legend>作品类型 <small>按官方 NDC 分类归组</small></legend>
          <label v-for="c in result.categories" :key="c"
            ><input
              type="checkbox"
              v-model="categories"
              :value="c"
              @change="filterChanged"
            />{{ c }}</label
          >
        </fieldset>
        <fieldset class="source-filters" :disabled="importing || loading">
          <legend>文字版本</legend>
          <label v-for="o in result.orthographies" :key="o"
            ><input
              type="checkbox"
              v-model="orthographies"
              :value="o"
              @change="filterChanged"
            />{{ o }}</label
          >
        </fieldset>
        <div class="source-list-toolbar">
          <span class="muted"
            >{{ offset + (result.works.length ? 1 : 0) }}–{{
              offset + result.works.length
            }}
            / {{ result.workTotal }}</span
          ><button
            class="text-link"
            type="button"
            :disabled="importing || loading"
            @click="selectPage"
          >
            勾选本页（最多 10 部）
          </button>
        </div>
        <div class="source-works" :aria-busy="loading">
          <div
            v-for="w in result.works"
            :key="w.id"
            class="source-work"
            :class="{ 'is-selected': selected[w.id] }"
          >
            <label
              ><input
                type="checkbox"
                :aria-label="'选择 ' + w.title + ' ' + w.orthography"
                :checked="!!selected[w.id]"
                :disabled="importing || loading || !w.htmlUrl || !!existing(w)"
                @change="toggle(w, ($event.target as HTMLInputElement).checked)"
              /><span
                ><strong lang="ja">{{ w.title }}</strong
                ><small v-if="w.subtitle" lang="ja">{{ w.subtitle }}</small
                ><small
                  >{{ w.category }} · {{ w.orthography }} ·
                  {{ w.ndc || "无分类" }}</small
                ></span
              ></label
            >
            <div class="source-work-actions">
              <button
                v-if="existing(w)"
                class="text-link"
                type="button"
                :disabled="importing"
                @click="emit('open', existing(w)!)"
              >
                开始阅读 ↗</button
              ><span v-else-if="!w.htmlUrl" class="muted">无 XHTML 正文</span
              ><a :href="w.url" target="_blank" rel="noopener noreferrer"
                >原书卡 ↗</a
              >
            </div>
            <p
              v-if="status[w.id]"
              class="source-work-status"
              :class="{ 'form-error': status[w.id].failed }"
              role="status"
            >
              {{ status[w.id].message }}
            </p>
          </div>
          <p v-if="!result.works.length" class="source-empty">
            此筛选下暂无作品。可以勾选其他类型或文字版本。
          </p>
        </div>
        <div class="source-pages">
          <button
            type="button"
            class="text-link"
            :disabled="offset === 0 || importing || loading"
            @click="
              offset -= 60;
              load();
            "
          >
            ← 上一页</button
          ><button
            type="button"
            class="text-link"
            :disabled="offset + 60 >= result.workTotal || importing || loading"
            @click="
              offset += 60;
              load();
            "
          >
            下一页 →
          </button>
        </div>
      </template>
      <details v-if="selectedCount" class="source-selection">
        <summary>已选 {{ selectedCount }} 部（跨作者保留）</summary>
        <div v-for="w in Object.values(selected)" :key="w.id">
          <span>{{ w.title }} · {{ w.authors.join("、") }}</span
          ><button
            class="text-link"
            type="button"
            :disabled="importing"
            @click="delete selected[w.id]"
          >
            移除
          </button>
        </div>
      </details>
      <div class="source-cache">
        <span>目录缓存 · {{ cachedDate }}</span
        ><button
          class="text-link"
          type="button"
          :disabled="importing || loading"
          @click="load(true)"
        >
          更新目录
        </button>
      </div>
    </template>
    <div class="source-bottom">
      <p v-if="current" class="source-warning" role="status">
        正在导入：{{ selected[current]?.title }}
      </p>
      <p class="muted">
        正文逐部获取，间隔至少 2
        秒。导入后可离线阅读；语法、词汇解读仍可手动或用解析 JSON 添加。
      </p>
      <div class="dialog-actions">
        <button
          v-if="importing"
          class="button secondary"
          type="button"
          :disabled="cancel"
          @click="cancel = true"
        >
          {{ cancel ? "当前作品完成后停止" : "停止后续导入" }}</button
        ><button
          class="button primary"
          type="button"
          :disabled="!selectedCount || importing || loading"
          @click="importSelected"
        >
          {{ importing ? "正在导入…" : `导入所选 ${selectedCount} 部 →` }}
        </button>
      </div>
    </div>
  </section>
</template>
