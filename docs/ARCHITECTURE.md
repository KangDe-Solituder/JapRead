# 阅读小版本：接口与数据约定

此文描述实际实现。路线见 `TODO.md`；不以本地忽略的 Demo 或 DESIGN.md 作为运行依赖。

## 代码边界

- `src/App.vue`：页面与界面状态；`SentenceView.vue`：按范围渲染标注与注音，正文使用 Vue 文本转义，无原始 HTML 注入。
- `src/api.ts`：类型化业务入口。桌面使用 Tauri `request` 命令，本地 Web 使用 POST `/api/{op}`。
- `crates/core`：共享业务校验与 SQLite；不依赖 Tauri 或 HTTP。
- `crates/sources`：共用 Application 路由、青空文库目录缓存／下载／正文提取；来源锁与数据库锁分开，网络不持有 SQLite 锁。
- `src-tauri`：Windows 壳、应用数据目录和命令适配。
- `crates/local-server`：仅用于开发的 loopback HTTP 适配器，不是可直接部署的 Linux 服务。

## 模型与边界

作品类型：文章、新闻、小说。均包含章节，章节包含句子；句子保留段落序号。导入时统一换行符、忽略空白行，按日文句末标点及关闭引号进行初步分句。标题、作者、URL、正文版本与各层 UUID 持久化。初版不允许修改正文，不做重新分句迁移。

SQLite schema v1 使用三张表：documents（作品与正文 JSON）、annotations（按作品关联的语境标注 JSON）、progress（每部作品的最近位置 JSON）。使用外键、事务、WAL 和 busy timeout。标注按出现位置保存；尚未建立跨作品全局词条。

标注锚点为 `documentId + version + sentenceId + [start,end)`。偏移采用 **Unicode scalar value**，Rust `chars()` / JavaScript `Array.from()`；不是 UTF-8 字节、UTF-16 code unit 或视觉字形。选择时剔除 `rt/rp` 注音节点。换行与分句后的持久化 sentence.text 是唯一定位基准。

重叠标注可以保存。正文优先显示当前选中标注，其余可从侧栏选中。只有完整落在一个渲染片段上的读音才展示注音，避免把整词读音错误分配给部分字符。跨句划词暂不支持。

## 业务操作

| op | payload | 返回 |
| --- | --- | --- |
| snapshot | `{}` | documents、annotations、progress（最近阅读优先） |
| import_document | title、author、kind、url、chapters: [{title,text}] | 已保存作品 |
| save_annotation | 完整 Annotation；新增 id 为空，编辑 id 必须已存在且属于同一作品 | 已保存标注 |
| set_learning | id、learn | 已保存标注 |
| import_annotations | ParsePackage | added 数量 |
| save_progress | documentId、chapterId、sentenceId | 包含服务器 updatedAt 毫秒时间的阅读位置 |

字段详情以 `src/types.ts` 和 Rust serde 定义为准；对象键采用 camelCase。Tauri 返回 Result 错误，HTTP 返回非 2xx 与 `{error}`。SQLite 是最终持久化来源。

## 助手解析交换格式 v1

阅读器的「导出解析模板」生成源正文、规则、空 responseTemplate 和单条字段示例。助手应仅返回 responseTemplate 形状的 JSON：

```json
{
  "schemaVersion": 1,
  "documentId": "从模板复制",
  "version": 1,
  "annotations": [{
    "id": "",
    "documentId": "从模板复制",
    "version": 1,
    "sentenceId": "从模板复制",
    "start": 0,
    "end": 1,
    "quote": "与该范围的原文完全一致",
    "kind": "词汇",
    "reading": "",
    "meaning": "非空的语境释义",
    "explanation": "",
    "learn": true
  }]
}
```

先前端预览，再后端完整字段、作品、版本、位置与长度校验；整批有效才事务写入。新增标注由后端生成 ID，默认 learn=true。相同作品、句子、范围和类型视为重复，保留已有记录，不覆盖人工解释或学习选择。需要修改已有记录时使用编辑入口。

## 运行与安全边界

Windows 数据保存在 Tauri app_data_dir 下 `com.japread.desktop/japread.sqlite3`。开发 HTTP 服务保存在仓库 `.local/japread.sqlite3`，二者独立。源码和 lockfile 进入 Git，数据库与构建产物不进入 Git。

本地 HTTP 仅绑定 127.0.0.1:1421，无 CORS 放行，检查浏览器 Origin，只接受 JSON；Vite 在 1420 端口代理。不要把该服务绑定公网或当作正式 Linux 部署。正式 Web 模式需要认证、授权、TLS 和独立配置。

本版仅在青空文库来源入口按需联网；没有 LLM、WebDAV，也不保存任何服务密钥。来源链接仅接受 HTTP/HTTPS。单次正文导入限制 5 MB、500 章；HTTP 请求上限 8 MB。当前 snapshot 一次读取全部正文，不能视为已经完成大书库性能设计。

## 设置、解析与备份接口 v1

`crates/sources/src/settings.rs` 使用 keyring 的 Windows native 后端，服务名 `com.japread.desktop` / `com.japread.development`，条目 `llm`、`webdav`；完整连接配置序列化为 UTF-8 secret blob（最多 2400 字节）。不持久化到数据库／前端；读取给界面时清空 API Key／密码，单独提供存在标记。高级 JSON 参数使用白名单，不允许覆盖正文、模型、鉴权或输出格式。只允许 HTTPS，HTTP 仅限 localhost / loopback；拒绝 URL 内嵌凭据、query、fragment；不跟随外部服务重定向。

| op | payload | 作用 |
| --- | --- | --- |
| settings_get | `{}` | 脱敏连接配置、密钥存在标记、secureStorage |
| settings_save | section=llm/webdav、config | 校验后写系统凭据；空密码保留旧值 |
| settings_clear | section | 移除该应用分区的系统凭据 |
| llm_test | `{}` | 固定短句测试请求与输出格式；不写书库 |
| llm_analyze | documentId、chapterId、sentenceIds | 从数据库取本章正文；返回 package、elapsedMs、usage，不自动保存 |
| webdav_test | `{}` | PROPFIND Depth 0 检查入口；不创建目录 |
| webdav_upload | `{}` | 获取一致的书库 snapshot，MKCOL 后 PUT 独立备份，返回 filename / bytes |
| webdav_list | `{}` | PROPFIND Depth 1，当前目录最多返回 100 个匹配备份 |
| webdav_restore | filename | 下载并校验后事务合并，返回 documents / annotations / conflicts |

LLM 与 WebDAV 使用独立的请求锁，网络等待期间不持有数据库锁。LLM 单批 1–8 句、最多 4000 Unicode 字符，回复最多 2 MB；接口支持 Chat Completions 及 Responses，后者设置 store=false。模型输出 quote + occurrence，由 Rust 在源句定位成 Unicode scalar 范围，再走 core 的版本与引用校验。截断、未知句子、引用不符或字段超限均拒绝整批。服务端错误正文与 URL 不返回前端，避免回显凭据。只允许用户显式重试，不自动重复计费请求。

`AnalysisPanel.vue` 串行处理所选章节，失败保留已完成批次，继续按钮从失败批次开始；暂停在当前请求完成后生效。结果逐条勾选后走已有 import_annotations 事务与同位置去重规则。不是持久后台任务；关闭预览丢弃未保存结果。

WebDAV 备份封装 `{app:"JapRead",schemaVersion:1,createdAt,snapshot}`，最大 50 MB。文件名含时间和 UUID，PUT 使用 If-None-Match:*；不直接复制正在使用的 SQLite。只列出同源同目录的 `japread-*.json`，拒绝目录穿越。下载后先验证所有正文／注音／标注／进度，再开事务；同 ID 正文冲突或同来源不同 ID 跳过，已有笔记及进度不覆盖。不传播删除、不合并人工修改、不自动同步，不备份系统凭据／外观设置／HTML 缓存。

## 后续演进

先补分页／按章加载、结构化数据库迁移和交互回归，再完善来源与后台任务。后台任务需独立于页面生命周期；前端不能把模型请求当作一个长期阻塞的按钮动作。同步前先制定版本、删除标记及人工修改合并协议。

## 内容来源接口 v1

桌面 IPC 和开发 HTTP 均经过 `japread_sources::Application`。耗时请求运行于 blocking worker；同一进程中来源请求串行，普通数据库阅读不等待网络锁。RSS 预留 provider，未启用。

| op | payload | 返回 |
| --- | --- | --- |
| source_providers | `{}` | `[{id,name,enabled}]`；aozora=true，rss=false |
| source_catalog | provider=aozora；可选 query、authorId、workQuery、categories、orthographies、offset、refresh | authors（最多100）、authorTotal、works（最多60）、workTotal、cachedAt、warning、筛选选项 |
| source_import | provider=aozora、workId | `{document,existing}` |

categories / orthographies 省略表示全部，空数组表示无匹配。用户不能传入下载 URL；正文地址只从官方目录取，校验 HTTPS + www.aozora.gr.jp:443，不跟随重定向。目录 ZIP 最大 10 MB、解压 CSV 最大 40 MB；作品下载最大 10 MB，解码后仍通过核心的正文 5 MB／500 章限制。连接超时 15 秒，总超时 45 秒，无自动重试；请求至少间隔 2 秒；429/503 读取整数 Retry-After，否则等待 60 秒，最多一小时。

目录缓存为 `sources/aozora/catalog.zip`，24 小时有效，手动更新间隔至少十分钟；过期更新失败时使用旧目录，十分钟内不自动再次尝试。同目录 `{workId}.html` 保存所选作品的原始正文快照，重试复用。桌面 sources 在 app_data_dir，Web 开发版在 `.local/sources`。没有缓存时联网失败显示错误，不创建空作品。

NDC 913 归小说，914 随笔／评论，911 诗歌，912 戏剧，其余归其他；这是浏览筛选，非文学体裁推断。入库顶层类型中小说对应小说，其余对应文章。此分类尚未细分外国文学。

Document 新增可选 `source`，保留 key=`aozora:{workId}`、来源、NDC、文字版本、原页、更新时间、底本／录入校对说明和导入提示。存储继续使用 schema v1 的 JSON；缺少新字段的旧作品仍可读取。同一来源 key 再次导入返回已有作品，禁止静默更新正文。当前去重在共享 Store 锁内完成，尚未保证不同进程同时操作同一数据库时的来源唯一约束。

Sentence 新增可选 `readings: [{start,end,reading}]`，偏移与标注一样按 Unicode scalar 计数；由章节原文范围映射到分句后位置，独立于 annotations，不增加学习／测验条目。用户标注读音优先于重叠的原书读音；被切开的注音不强行分配。提取 `.main_text`，跳过 script/style/rt/rp 等，分章依据 h2–h5，标题线性排列；无标题时保留一个“正文”章。原网页布局、复杂卷章层级、图片字图片尚未复刻，图片按 alt 文本保存并提示。

前端导入批次最多 10 部，逐部提交和持久化。失败即暂停，失败与未开始项保持勾选；停止按钮在当前作品结束后生效。批次运行期间锁定关闭弹窗，避免界面销毁后仍继续请求；没有跨应用重启的任务队列。已导入作品可离线阅读。
