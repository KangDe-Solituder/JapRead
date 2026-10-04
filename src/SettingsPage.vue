<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { api, desktop } from "./api";
import type { ConnectionSettings, LlmConfig } from "./types";
import {
  theme,
  fontSize,
  motion,
  palette,
  palettes,
  animatePage,
} from "./preferences";
const content = ref<HTMLElement>();
const emit = defineEmits<{ updated: [] }>();
const tab = ref("appearance"),
  config = ref<ConnectionSettings>(),
  busy = ref(false),
  error = ref(""),
  status = ref("");
const backups = ref<string[]>([]),
  chosenBackup = ref("");
const savedLlm = ref(""),
  savedDav = ref("");
const profileDialog = ref<HTMLDialogElement>();
const profileOpen = ref(false);
let disposed = false;
const profileId = ref<string | null>(null),
  profileName = ref("");
const profileHasKey = ref(false),
  clearProfileKey = ref(false);
const llmDraft = ref<LlmConfig>({
  endpoint: "",
  model: "",
  protocol: "chat",
  apiKey: "",
  timeoutSeconds: 90,
  maxTokens: 6000,
  structured: false,
  extraBody: "",
});
async function editProfile(
  profile?: NonNullable<typeof config.value>["llmProfiles"][number],
) {
  profileId.value = profile?.id ?? null;
  profileName.value = profile?.name ?? "";
  profileHasKey.value = profile?.hasApiKey ?? false;
  clearProfileKey.value = false;
  llmDraft.value = profile
    ? { ...profile.config, apiKey: "" }
    : {
        endpoint: "",
        model: "",
        protocol: "chat",
        apiKey: "",
        timeoutSeconds: 90,
        maxTokens: 6000,
        structured: false,
        extraBody: "",
      };
  error.value = "";
  profileOpen.value = true;
  await nextTick();
  if (!disposed && profileOpen.value && !profileDialog.value?.open)
    profileDialog.value?.showModal();
}
function closeProfile() {
  if (profileDialog.value?.open) profileDialog.value.close();
  profileOpen.value = false;
  llmDraft.value.apiKey = "";
  error.value = "";
}
onBeforeUnmount(() => {
  disposed = true;
  closeProfile();
});
async function saveProfile() {
  apply(
    await api.saveLlmProfile(
      profileId.value,
      profileName.value,
      llmDraft.value,
      clearProfileKey.value,
    ),
    "llm",
  );
  closeProfile();
  status.value = "配置已保存到 Windows 凭据管理器。";
}
watch(
  tab,
  () => {
    closeProfile();
    animatePage(content.value || null);
  },
  { flush: "post" },
);
const tabs = [
  { id: "appearance", name: "阅读与外观" },
  { id: "llm", name: "LLM 解析" },
  { id: "webdav", name: "WebDAV" },
  { id: "about", name: "本机数据" },
];
function apply(value: ConnectionSettings, section?: "llm" | "webdav") {
  if (section && config.value) {
    if (section === "llm") {
      config.value.llm = value.llm;
      config.value.hasApiKey = value.hasApiKey;
      config.value.llmProfiles = value.llmProfiles;
      config.value.activeLlmId = value.activeLlmId;
      savedLlm.value = JSON.stringify(value.llm);
    } else {
      config.value.webdav = value.webdav;
      config.value.hasDavPassword = value.hasDavPassword;
      savedDav.value = JSON.stringify(value.webdav);
      backups.value = [];
      chosenBackup.value = "";
    }
  } else {
    config.value = value;
    savedLlm.value = JSON.stringify(value.llm);
    savedDav.value = JSON.stringify(value.webdav);
  }
}
async function perform(fn: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  status.value = "";
  try {
    await fn();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}
async function save(section: "llm" | "webdav") {
  if (config.value) {
    apply(await api.saveSettings(section, config.value[section]), section);
    status.value =
      "已保存到 Windows 凭据管理器。密码输入框已清空，后续留空表示保留。";
  }
}
function dirty(section: "llm" | "webdav") {
  return (
    !!config.value &&
    JSON.stringify(config.value[section]) !==
      (section === "llm" ? savedLlm.value : savedDav.value)
  );
}
function ready(section: "llm" | "webdav") {
  if (dirty(section))
    throw new Error("连接配置有未保存的修改，请先保存再测试或使用。");
}
async function test(section: "llm" | "webdav") {
  ready(section);
  status.value = (
    await (section === "llm" ? api.testLlm() : api.testWebdav())
  ).message;
}
async function clear(section: "llm" | "webdav") {
  apply(await api.clearSettings(section), section);
  status.value = "已移除此服务的本机连接配置。";
}
async function upload() {
  ready("webdav");
  const result = await api.uploadBackup();
  status.value = `已上传独立备份（${Math.ceil(result.bytes / 1024)} KB），原有备份保留。`;
  backups.value = await api.listBackups();
}
async function list() {
  ready("webdav");
  backups.value = await api.listBackups();
  status.value = backups.value.length
    ? `找到 ${backups.value.length} 份备份。`
    : "远程尚无 JapRead 备份，可先上传一份。";
}
async function restore() {
  ready("webdav");
  const result = await api.restoreBackup(chosenBackup.value);
  status.value = `已新增 ${result.documents} 部作品、${result.annotations} 条笔记；跳过 ${result.conflicts} 部存在版本或来源冲突的作品。已有笔记与进度保留。`;
  emit("updated");
}
onMounted(() => perform(async () => apply(await api.settings())));
</script>
<template>
  <div class="page-heading">
    <div>
      <span class="eyebrow">A SPACE OF YOUR OWN</span>
      <h1>设置</h1>
      <p class="muted">让阅读、解析和保存，按你的习惯运作。</p>
    </div>
  </div>
  <div class="preferences-layout">
    <nav class="preferences-nav" aria-label="设置分类">
      <button
        v-for="t in tabs"
        :key="t.id"
        :class="{ active: tab === t.id }"
        :aria-current="tab === t.id ? 'page' : undefined"
        @click="
          tab = t.id;
          error = '';
          status = '';
        "
        :disabled="busy"
      >
        {{ t.name }}
      </button>
    </nav>
    <div ref="content" class="preferences-content">
      <p v-if="error" class="form-error preference-message" role="alert">
        {{ error }}
      </p>
      <p v-if="status" class="preference-message" role="status">{{ status }}</p>
      <p v-if="busy" class="muted" role="status">正在处理，请稍候…</p>
      <template v-if="tab === 'appearance'">
        <section class="preference-card appearance-studio">
          <header>
            <span class="eyebrow">APPEARANCE</span>
            <h2>阅读的底色</h2>
            <p>选一种颜色，留一处安静读书的地方。</p>
          </header>
          <div class="palette-workbench">
            <div class="palette-controls">
              <div class="palette-heading">
                <h3>主题色系</h3>
                <span class="muted">明暗皆宜</span>
              </div>
              <div class="palette-options" aria-label="主题色系">
                <button
                  v-for="p in palettes"
                  :key="p.id"
                  :aria-pressed="palette === p.id"
                  :class="{ selected: palette === p.id }"
                  @click="palette = p.id"
                >
                  <span class="palette-dot" :style="{ background: p.swatch }"
                    ><span v-if="palette === p.id">✓</span></span
                  >
                  <span
                    ><strong>{{ p.name }}</strong
                    ><small>{{ p.note }}</small></span
                  >
                </button>
              </div>
              <div class="theme-options" aria-label="明暗模式">
                <button
                  v-for="t in [
                    { id: 'light', name: '浅色', note: '明亮柔和' },
                    { id: 'dark', name: '暗夜', note: '适合夜读' },
                    { id: 'system', name: '跟随系统', note: '随系统切换' },
                  ]"
                  :key="t.id"
                  :class="{ selected: theme === t.id }"
                  :aria-pressed="theme === t.id"
                  @click="theme = t.id"
                >
                  <strong>{{ t.name }}</strong
                  ><small>{{ t.note }}</small>
                </button>
              </div>
            </div>
            <aside class="reading-preview" aria-label="主题阅读预览">
              <span class="eyebrow">A MOMENT TO READ</span>
              <h3 lang="ja">雨の日の読書</h3>
              <p lang="ja" :style="{ fontSize: fontSize + 'px' }">
                窓の外では、<mark
                  ><ruby>静<rt>しず</rt></ruby
                  >かな雨</mark
                >が降っている。<br />今日は、ゆっくり本を読もう。
              </p>
              <div class="preview-definition">
                <span>静かな雨 <small>しずかなあめ</small></span>
                <p>安静的雨，让阅读慢下来。</p>
              </div>
              <small class="muted">配色与字号即时预览</small>
            </aside>
          </div>
        </section>
        <div class="preference-pair">
          <section class="preference-card">
            <header>
              <h2>正文字号</h2>
              <p>与阅读器目录中的字号同步。</p>
            </header>
            <label class="setting-slider"
              >日文正文 <output>{{ fontSize }} px</output
              ><input
                type="range"
                min="16"
                max="32"
                step="1"
                v-model.number="fontSize"
                aria-label="设置正文字号"
            /></label>
            <p
              class="font-preview"
              lang="ja"
              :style="{ fontSize: fontSize + 'px' }"
            >
              <ruby>読<rt>よ</rt></ruby
              >むほど、世界がひろがる。
            </p>
            <button class="text-link" @click="fontSize = 20">恢复 20 px</button>
          </section>
          <section class="preference-card">
            <header>
              <h2>动画速度</h2>
              <p>页面切换、设置栏目与抽屉共用此速度。</p>
            </header>
            <label class="field-label"
              >速度<select v-model="motion">
                <option value="fast">轻快</option>
                <option value="normal">标准</option>
                <option value="slow">舒缓</option>
                <option value="off">关闭动画</option>
              </select></label
            >
            <p class="muted preference-note">
              自动尊重系统“减少动态效果”设置。
            </p>
          </section>
        </div>
        <p class="muted preference-note">
          外观设置即时生效，自动保存在这台设备。
        </p>
      </template>
      <template v-else-if="tab === 'llm' && config">
        <section class="preference-card">
          <header>
            <span class="eyebrow">UNDERSTAND IN CONTEXT</span>
            <h2>连接你的语言模型</h2>
            <p>阅读时划选词句获取解读，理解后再选择保留。</p>
          </header>
          <div class="llm-profile-heading">
            <p class="muted">
              {{
                config.llmProfiles.length
                  ? "选择一套配置用于后续解析。"
                  : "还没有模型配置，添加第一套即可开始。"
              }}
            </p>
            <button
              class="button primary"
              :disabled="busy || !config.secureStorage"
              @click="editProfile()"
            >
              ＋ 添加配置
            </button>
          </div>
          <div class="llm-profile-grid">
            <article
              v-for="p in config.llmProfiles"
              :key="p.id"
              class="llm-profile-card"
              :class="{ active: config.activeLlmId === p.id }"
            >
              <button
                class="profile-select"
                :disabled="busy"
                :aria-pressed="config.activeLlmId === p.id"
                @click="
                  perform(async () => {
                    apply(await api.activateLlmProfile(p.id), 'llm');
                    status = '已切换模型配置。';
                  })
                "
              >
                <span class="tag">{{
                  config.activeLlmId === p.id ? "正在使用" : "点击启用"
                }}</span>
                <h3>{{ p.name }}</h3>
                <p>{{ p.config.model }}</p>
                <small
                  >{{
                    p.config.protocol === "chat" ? "兼容接口" : "Responses"
                  }}
                  · {{ p.hasApiKey ? "密钥已托管" : "无 API Key" }}</small
                >
              </button>
              <div class="settings-actions">
                <button
                  class="text-button"
                  :disabled="busy"
                  @click="editProfile(p)"
                >
                  编辑
                </button>
                <button
                  class="text-button"
                  :disabled="busy"
                  @click="
                    perform(async () => {
                      apply(await api.removeLlmProfile(p.id), 'llm');
                      status = '已移除配置；若移除当前配置，请另选一套。';
                    })
                  "
                >
                  移除
                </button>
              </div>
            </article>
          </div>
          <div class="settings-actions">
            <button
              class="button secondary"
              :disabled="busy || !config.activeLlmId"
              @click="perform(() => test('llm'))"
            >
              测试当前配置
            </button>
          </div>
          <p class="muted preference-note">
            测试会向当前模型发送一句日文。每套连接资料分别保存在 Windows
            凭据管理器，不进入书库或 WebDAV 备份。
          </p>
        </section>
      </template>
      <template v-else-if="tab === 'webdav' && config">
        <section class="preference-card">
          <header>
            <span class="eyebrow">KEEP YOUR READING</span>
            <h2>WebDAV 连接</h2>
            <p>将书库、笔记和进度保存到你自己的空间。</p>
          </header>
          <form @submit.prevent="perform(() => save('webdav'))">
            <fieldset
              :disabled="busy || !config.secureStorage"
              class="settings-fields"
            >
              <label class="field-label"
                >WebDAV 服务地址<input
                  v-model="config.webdav.endpoint"
                  type="url"
                  required
                  placeholder="https://服务器/WebDAV入口"
                  autocomplete="off"
              /></label>
              <div class="form-row">
                <label class="field-label"
                  >用户名<input
                    v-model="config.webdav.username"
                    autocomplete="off" /></label
                ><label class="field-label"
                  >密码／应用密码
                  <span class="credential-state">{{
                    config.hasDavPassword ? "已安全保存" : ""
                  }}</span
                  ><input
                    v-model="config.webdav.password"
                    type="password"
                    autocomplete="new-password"
                    placeholder="留空保留已保存的密码"
                /></label>
              </div>
              <label class="field-label"
                >远程子目录<input
                  v-model="config.webdav.remotePath"
                  placeholder="JapRead"
              /></label>
              <div class="settings-actions">
                <button class="button primary">保存 WebDAV 配置</button
                ><button
                  class="button secondary"
                  type="button"
                  :disabled="!config.webdav.endpoint || dirty('webdav')"
                  @click="perform(() => test('webdav'))"
                >
                  测试连接
                </button>
              </div>
            </fieldset>
          </form>
          <button
            v-if="config.webdav.endpoint"
            class="text-link remove-config"
            :disabled="busy"
            @click="perform(() => clear('webdav'))"
          >
            移除此连接配置
          </button>
        </section>
        <section class="preference-card">
          <header>
            <h2>手动备份与合并</h2>
            <p>
              每次上传保留一个独立版本。合并仅补充缺失内容，已有笔记、进度和冲突版本保留本地记录。
            </p>
          </header>
          <div class="settings-actions">
            <button
              class="button primary"
              :disabled="busy || !config.webdav.endpoint || dirty('webdav')"
              @click="perform(upload)"
            >
              上传当前书库备份</button
            ><button
              class="button secondary"
              :disabled="busy || !config.webdav.endpoint || dirty('webdav')"
              @click="perform(list)"
            >
              查看远程备份
            </button>
          </div>
          <div v-if="backups.length" class="backup-selection">
            <label class="field-label"
              >选择备份<select v-model="chosenBackup">
                <option value="">请选择…</option>
                <option v-for="b in backups" :key="b" :value="b">
                  {{ b }}
                </option>
              </select></label
            ><button
              class="button secondary"
              :disabled="busy || !chosenBackup || dirty('webdav')"
              @click="perform(restore)"
            >
              合并所选备份到本机
            </button>
          </div>
          <p class="muted preference-note">
            只备份阅读数据，不包含 LLM／WebDAV
            配置、密码或原网页缓存。此版本尚未启用自动双向同步。
          </p>
        </section>
      </template>
      <template v-else-if="tab === 'about'"
        ><section class="preference-card">
          <header><h2>数据留在你的设备</h2></header>
          <p>{{ desktop ? "Windows 桌面版" : "本地 Web 开发版" }} · SQLite</p>
          <p class="muted preference-note">
            {{
              desktop
                ? "书库位于系统应用数据目录 com.japread.desktop。"
                : "书库位于项目 .local/japread.sqlite3，与桌面版独立。"
            }}连接配置保存在独立的 Windows
            凭据条目中，开发版与桌面版互不读取对方的配置。
          </p>
          <p class="muted preference-note">
            青空文库导入、手工与 JSON 解读可以离线使用已有数据。LLM
            请求仅在你点击测试或开始解析后发送。
          </p>
        </section></template
      >
      <p v-if="config && !config.secureStorage" class="form-error">
        当前系统不支持 Windows
        凭据保存；连接配置功能不可用，不会回退到明文文件。
      </p>
    </div>
  </div>

  <Teleport to="body">
    <template v-if="config">
      <dialog
        v-if="profileOpen"
        ref="profileDialog"
        class="llm-profile-dialog"
        @close="closeProfile"
        @cancel.prevent="!busy && closeProfile()"
      >
        <div class="dialog-heading">
          <h2>{{ profileId ? "编辑模型配置" : "添加模型配置" }}</h2>
          <button
            class="close-button"
            :disabled="busy"
            aria-label="关闭模型配置"
            @click="closeProfile"
          >
            ×
          </button>
        </div>
        <form @submit.prevent="perform(saveProfile)">
          <label class="field-label"
            >配置名称<input
              v-model="profileName"
              required
              maxlength="40"
              :disabled="busy"
              placeholder="例如：本地模型 / 日常解析"
          /></label>
          <fieldset
            :disabled="busy || !config.secureStorage"
            class="settings-fields"
          >
            <div class="form-row">
              <label class="field-label"
                >接口类型<select v-model="llmDraft.protocol">
                  <option value="chat">Chat Completions（兼容接口）</option>
                  <option value="responses">OpenAI Responses</option>
                </select></label
              ><label class="field-label"
                >模型名称<input
                  v-model="llmDraft.model"
                  required
                  placeholder="填写服务商提供的模型 ID"
                  autocomplete="off"
              /></label>
            </div>
            <label class="field-label"
              >接口地址<input
                v-model="llmDraft.endpoint"
                required
                type="url"
                placeholder="https://服务地址/v1"
                autocomplete="off"
            /></label>
            <label class="field-label"
              >API Key
              <span class="credential-state">{{
                profileHasKey
                  ? "已安全保存"
                  : "尚未保存（本地免鉴权服务可留空）"
              }}</span
              ><input
                v-model="llmDraft.apiKey"
                type="password"
                autocomplete="new-password"
                :placeholder="
                  profileHasKey
                    ? '留空保留已保存的密钥'
                    : '填入密钥，保存后由 Windows 保管'
                "
            /></label>
            <div class="form-row">
              <label class="field-label"
                >超时（秒）<input
                  v-model.number="llmDraft.timeoutSeconds"
                  type="number"
                  min="15"
                  max="180"
                  required /></label
              ><label class="field-label"
                >每批输出上限（tokens）<input
                  v-model.number="llmDraft.maxTokens"
                  type="number"
                  min="512"
                  max="16000"
                  required
              /></label>
            </div>
            <label class="learn-checkbox"
              ><input v-model="llmDraft.structured" type="checkbox" />启用严格
              JSON Schema（需要模型支持）</label
            >
            <details class="settings-advanced">
              <summary>高级推理参数</summary>
              <label class="field-label"
                >附加 JSON<textarea
                  v-model="llmDraft.extraBody"
                  rows="4"
                  placeholder='例如 {"temperature": 0.2}；留空使用服务默认值'
                ></textarea>
              </label>
              <p class="muted">
                可设置 reasoning、thinking、enable_thinking
                等服务参数；不覆盖模型、正文与输出格式。
              </p>
            </details>
            <label v-if="profileHasKey" class="learn-checkbox"
              ><input
                type="checkbox"
                v-model="clearProfileKey"
              />清除已保存密钥（用于免鉴权服务）</label
            >
            <p v-if="error" class="form-error" role="alert">{{ error }}</p>
            <div class="settings-actions">
              <button class="button primary">保存配置</button
              ><button
                type="button"
                class="button secondary"
                @click="closeProfile"
              >
                取消
              </button>
            </div>
          </fieldset>
        </form>
      </dialog>
    </template>
  </Teleport>
</template>
