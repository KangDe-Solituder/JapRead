# JapRead 接力说明

更新：2026-10-04。给用户和 Joshua 的接力入口。用户已授权将当前阅读版本提交并推送 GitHub；实际提交以 Git 历史为准。本轮没有向其他助手发送消息。

仓库交付范围：源码、正式图标、原创示例、依赖清单与锁文件。`.gitignore` 排除设计／demo／图标候选稿、npm 依赖及缓存、Rust／前端产物、本地数据库／备份／日志、环境文件及常见凭据文件。提交前扫描未发现真实凭据；命中项均为隔离测试假值或 URL 拒绝用例。LLM／WebDAV 的实际连接配置仍留在 Windows 凭据管理器。规则不能防止手动把密钥写入源码，后续提交仍需检查暂存差异。

## 先读这些

1. 根目录 `README.md`：运行方法、产品长期目标。
2. `TODO.md`：本轮完成状态与后续范围。
3. `docs/ARCHITECTURE.md`：实际代码边界、接口和锚点协议。

`DESIGN.md`、`demo/` 被 `.gitignore` 忽略。正式应用所有必需源码、样式、图标和示例都已独立放在可提交目录内，不依赖被忽略文件。视觉沿用 GPT Demo：纸张／墨绿、日文衬线、暗夜默认；只保留当前可用功能入口。

## 当前实际能力

- Vue + TypeScript + Vite 前端，Rust 共享业务核心，SQLite 持久化，Tauri 2 Windows 壳。
- 新闻／文章／小说正文粘贴导入；小说用独立 `---` 行分章。
- 青空文库作者目录、类型／文字版本复选、作品名搜索与分页、勾选后串行导入；原注音、章节与出处保留。
- 书库搜索与类型筛选、章节阅读、明暗主题、注音开关、默认收起的左右抽屉、字号调整（16–32 px，记忆选择）。
- 单句内划词，添加和编辑词汇／语法／读音解读；默认学习，支持取消。
- 解析模板复制、JSON 校验预览与事务导入。重复位置保留手工编辑和学习状态。
- 知识列表与原句回链，章节／句子级阅读位置恢复。
- 设置分区、Windows 凭据管理、LLM 按需划词解读与预览保存、WebDAV 手动独立备份／合并；章节批量解析入口已隐藏、暂列 TODO。

## 已执行的验证

- `npm run build`：TypeScript 与 Vite 生产构建通过。
- `cargo test -p japread-core`：3 组测试通过。覆盖 Unicode（含 emoji）范围、错误范围／版本／来源 URL、无效批次不写入、重复导入保护、SQLite 关闭重开与阅读位置持久化。
- `cargo fmt --all --check`：通过。
- `npm run desktop:build`：成功生成内嵌前端的 `target/debug/japread.exe`。
- Windows EXE 已启动，存在响应中的「JapRead · 在阅读中，与日语相遇」窗口，应用数据目录创建 SQLite/WAL 文件。
- 共享前端 + Rust HTTP + SQLite 实际浏览器操作通过：导入原创两章短文、鼠标划选「静かな雨」并保存读音与解释、编辑已有解释、取消学习、导出模板、错误 JSON 被拦截、导入两条新标注并跳过已有一条、知识回链准确定位第二个「猫😀」、切换第二章、刷新后继续阅读恢复第二章和浅色偏好，再切回暗色。HTTP 服务重启后，已有正文、标注仍可读取并继续编辑。

验证边界：本轮浏览器交互验收使用真实 HTTP 适配与 SQLite；Windows 原生窗口只验证编译、启动和数据库初始化，尚未逐项点击 Tauri IPC 路径。不要把浏览器验收写成 Windows 全流程验收。显示缩放、多窗口、大书库和重叠注音尚需进一步测试。

## 下一步建议

优先按 TODO 的「现代日语内容来源」阶段推进：RSS / Atom 新闻候选列表与按需正文导入，调研现代文学／生活文章的合法来源，并评估无 DRM EPUB 导入。当前阅读流程已基本成形，全文解析继续暂缓，维持划词解读后选择保留。不要让用户把密钥发到聊天，也不要从其他项目提取配置。保留 Windows 原生交互、WebDAV 真实服务、分页及数据库迁移的待验收事项。

当前已知范围与限制：

- 没有 RSS、持久后台队列、测验、EPUB 或朗读。RSS 仅预留禁用的 provider；LLM 与 WebDAV 手动备份已接入，见下方补充。
- 不编辑／删除正文；WebDAV 可备份及保守合并，尚非完整覆盖还原。需要手工备份数据库时先关闭程序，避免漏掉 WAL 中的数据。
- 阅读位置保存为句子级，滚动停止后 250ms 异步写入；不是逐像素恢复，突然强制结束可能丢失最后一次尚未提交的位置。
- snapshot 仍读取全部作品；只按当前章节渲染。大数据前需拆列表摘要与按章正文查询。
- 每条知识当前是语境标注，未做跨作品同义词条合并；同位置同类型的 JSON 导入采取保留旧值策略。
- 分句采用轻量标点规则；导入会规范换行并省略空白行。未来改规则时必须引入正文版本迁移，不可重新分句覆盖已有 ID。
- 划选从注音 `rt` 起止时提示改选正文；跨句选择暂不支持。重叠标注仅在完整片段上显示注音。
- 模板目前通过文本框复制，不是系统保存文件对话框。助手必须返回解析对象，不能把整个导出模板原样导回。
- 本地 HTTP 适配无登录，仅绑定 loopback；正式 Linux 部署必须另做认证与权限方案。
- `src/visual.css` 保留了原 Demo 的基础样式，其中部分样式暂未使用；后续整理时不要随意改动视觉方向。

## 数据与环境

桌面库与本地 Web 库独立。验收短文只导入了 `.local/japread.sqlite3`；桌面首次启动为干净书库。运行时无 CDN 和外部字体依赖。

Windows 本机已安装 Node 22、Rust/MSVC 和 WebView2。新环境使用 lockfile：`npm ci`，Rust 使用 Cargo.lock。Linux 云端可先运行 `cargo test -p japread-core` 和前端构建，不必为核心测试安装桌面 WebKit 依赖。

不要提交 `target/`、`dist/`、`node_modules/`、`.local/`、任何 API/WebDAV 凭据或个人阅读数据。

## 阅读器调整补充

2026-10-03：目录和解读改为覆盖式抽屉，不占正文列宽。左右箭头独立收放，展开时反转；点击词句／知识回链会展开右侧，Esc 收起（弹窗打开时保留弹窗的 Esc 行为）。隐藏抽屉使用 inert，不进入键盘焦点。左侧加入字号滑杆、增减、恢复默认，保存于 japread.fontSize。

注音使用原生 ruby，每条标注的 reading 对应其完整 quote，支持单汉字多假名（猫／ねこ）；并未自动拆分混合词组的逐字读音。示例文件去掉测试 emoji；本机 Web 测试库中的原始验收短文也已修正，并备份为 .local/japread.before-sample-cleanup-1790993717.sqlite3。其正文版本升为 2，关联标注版本与范围同步修正；旧版本助手 JSON 需重新导出模板。桌面库没有这篇示例，未改动。

上述阅读器调整时未扩展导入；随后已接入青空文库，见下方记录。RSS／EPUB 留在 TODO。

本轮验证：前端类型检查和生产构建通过，Windows EXE 重建通过；浏览器实测单字猫／ねこ、抽屉收放与箭头状态、Esc、章节切换、16/32 px 边界、刷新后字号保留、明暗主题。展开解读前后正文宽度同为 919 px。验证完毕已关闭 JapRead Vite，确认 1420 无监听。

## 青空文库导入补充

2026-10-03：新增 `crates/sources`，桌面与 Web 共用 `Application`；网络请求在 blocking worker 执行，与 SQLite 锁分离。前端 `AozoraImport.vue` 放在现有导入弹窗，默认青空文库，保留粘贴正文；RSS 为未启用入口。

使用官方扩展 UTF-8 ZIP 目录，不抓作者／书卡页面。第一次获取元数据，后续本地搜索作者、作品名和 NDC／文字版本复选筛选；60 条分页、跨作者勾选、每批最多 10 部。只下载所选 XHTML，串行间隔至少 2 秒，缓存与冷却规则见 ARCHITECTURE。失败暂停本批次，未完成项保持勾选；停止在当前作品完成后生效。

保留正文分章、Unicode 字符范围的原书注音、书卡来源、底本／录入校对说明。原书注音放在 Sentence.readings，与 annotations 分离，不加入学习手帐；没有调用模型。同一来源作品重复导入返回已有文档，不覆盖进度和笔记。仅 TXT 的作品暂不可选；少量图片字用 alt 说明替代，复杂层级标题暂线性分章。

验收：`npm test` 共 7 项通过（核心 4、来源 3）；覆盖目录去重与作者读音检索、HTML 正文隔离／分章／注音、来源 URL 限制、Unicode 锚点、来源重复导入不换 ID 和旧有持久化测试。前端生产构建、Rust 格式检查、Windows 内嵌前端 EXE 构建通过。

浏览器通过真实 Rust HTTP + SQLite 操作导入夏目漱石《夢十夜》（10 章、665 句、714 处原注音；3 处图片字保留文字说明）；芥川竜之介《羅生門》通过 Rust 联网下载。已验证作者搜索、作品名筛选、类型／文字版本复选、导入状态、原注音开关、十章目录与第二章切换、服务重启后的读取和重复导入返回已有作品。知识手帐保持原有 3 条。浏览器无错误日志。

验收作品与目录只放在 `.local` Web 开发库／缓存，不写入用户桌面书库。截图 `.local/aozora-import.png` 不进入 Git。Windows 原生 IPC 完整点击流程、异常网络与源站全部格式仍未穷尽验证；不要将 Web 验收记录等同于原生全流程验收。

最终 EXE 已启动，窗口标题正确且 Responding=true，随后关闭测试窗口。调试 Vite／HTTP 服务已停止，确认 1420、1421 均无监听。未提交或推送 Git。

## 设置、LLM 与 WebDAV 补充（2026-10-03）

左下角学习者信息替换为设置。`SettingsPage.vue` 使用分类导航和卡片表单，参考 OshiNote 布局；`preferences.ts` 统一主题（含跟随系统）、阅读字号及动画速度，目录滑杆与设置同步。`AnalysisPanel.vue` 从阅读器进入，选择章节、分批请求、预览逐项保存。LLM 地址及高级参数参考 LiveCaption 的兼容接口设计，同时支持 Responses。

`settings.rs` 用 JapRead 自己的 Windows 凭据条目保管完整连接资料，密钥与密码不回显；开发版／桌面版完全分离。没有复制 OshiNote 或 LiveCaption 的实际配置，没有读取其他应用的凭据。字节存储使用 keyring set_secret / get_secret，避免 UTF-16 扩容导致凭据大小超限。

`llm.rs` 在后端构造请求，引用由服务端重新定位并校验，预览不写库。用户明确勾选保存后复用 core 导入事务，已有同位置同类型笔记不覆盖。关闭窗口会丢失未保存预览，暂停只能在本批完成后生效；持久任务／立即取消仍在 TODO。真实服务可能对推理参数和 token 字段有不同要求，须用用户配置验收。

`webdav.rs` 实现 PROPFIND / MKCOL / PUT / GET。独立版本 JSON 备份、最多 100 个候选、同目录文件名约束，合并前校验整包并事务写入；保留本地已有笔记、进度和冲突作品。没有自动同步、删除传播或冲突编辑。来源 HTML 缓存、连接配置与外观偏好不在备份内。

验证：Rust 共 13 项测试（核心 5、来源及服务 8）通过，含随机命名 Windows 凭据的真实保存／读取／清理、loopback HTTP 两种 LLM 协议、错误引用及认证失败、WebDAV 上传／列表／取回、重复合并、手工修订与进度保护、坏备份原子拒绝。测试只访问本机模拟服务，没有付费模型或真实 WebDAV 调用。

浏览器共享前端验收通过：设置入口、明暗主题、字号和关闭动画刷新保持、模型配置保存与连接测试、两句解析预览、取消其中一条后保存、阅读器及知识数量刷新；18 句分 3 批，暂停在 1/3 后保留 8 条，模拟断线显示错误，服务恢复后从失败批次完成 18 条。浏览器无 warn / error 日志。UI 测试库在 `.local/settings-ui/.local`，原有 `.local/japread.sqlite3` 和桌面库未植入测试内容；测试连接配置已移除。

前端生产构建、格式检查、Windows EXE 构建通过。原生窗口仍只做启动检查，完整 IPC 界面操作待用户验收。界面截图 `.local/japread-settings.png` 不进入 Git。

最终程序启动检查：窗口标题正确、Responding=true；已关闭测试 EXE、浏览器、模拟服务及 1420／1421 开发服务。本轮未提交或推送 Git。

### 设置布局修正

最新收尾：移除暖纸，旧 `warm-paper` 偏好自动迁移 `rainy-cafe`；明暗控件移到六色色系网格下方，左侧配置与右侧预览使用等宽列。浏览器实测左右 432.5 px，控件顺序正确，截图 `.local/japread-appearance-final.png`。Logo 概念与完整生成提示词在 `assets/logo-concepts/`，内置 image_gen 生成 A 书页伙伴、B 黑白阅读伙伴；尚待选择，不替换原 ICO。

本轮用户正在运行 `target/debug/japread.exe`，常规构建因文件占用未能覆盖，保留该用户进程。改用 `cargo rustc -p japread --bin japread --features custom-protocol -- -C extra-filename=-preview` 生成嵌入前端的独立预览程序；不要把被占用的旧 EXE 当成本轮新构建。正常构建可在用户退出原程序后再次运行。

独立产物已复制到 `target/debug/japread-preview.exe`；源产物在 `target/debug/deps/japread-preview.exe`。两种运行形态使用同一桌面数据目录，不要将预览版误认为隔离书库。

后续配色轮次：新增 `palettes.ts`（原墨绿 + OshiNote 六色系的明暗适配）、`themes.css`（覆盖原 Demo 的固定绿色到语义色），`preferences.ts` 保存 `japread.palette` 外观偏好。模式与色系独立；主卡片包含色系选择和阅读预览，下方字号／动画。`animatePage` 对主内容／设置栏目做 120/220/420 ms 淡入，off 或系统减少动态效果时不启用；快速切页结束旧动画，不延迟正文挂载或改变 fixed 抽屉坐标。实际浏览器观察到 slow 模式中间透明度、off 模式透明度为 1，14 种配色组合切换及刷新保留通过。运行时系统减少动态效果分支已实现，未修改用户 Windows 系统设置作测试。

再次复核安全配置：`settings.rs` 完整连接资料只用 Windows native keyring 的 set_secret/get_secret；public 清空密钥／密码，WebDAV 上传只封装 Store snapshot。页面组件没有将连接资料写入 localStorage；主题、色系、字号、动画属于非敏感外观偏好。未读取用户实际凭据。新截图 `.local/japread-colors.png`。

本轮最终验证：TypeScript / Vite 与 Windows EXE 重建通过；隔离凭据及模拟 LLM/WebDAV 往返测试复测通过。阅读器正文与解读抽屉已验收雾蓝配色，去除残留绿色背景，浏览器无 warn/error。测试结束恢复开发浏览器暗夜／墨绿／标准动画，关闭 1420／1421。未提交或推送。

用户指出应用侧栏加设置侧栏造成三列嵌套，现改为设置标题下方的横向文字栏目，以加粗和下划线标识当前栏目；所有设置内容占用下方完整宽度，保留卡片与暗夜视觉。仅调整 `SettingsPage.vue` 栏目展示和 `app.css` 布局，连接及保存逻辑不变。四个栏目切换、实际界面、前端构建与 Windows EXE 重建已验证；截图 `.local/japread-settings-tabs.png`。测试使用隔离书库，完成后关闭 1420／1421，未提交或推送。

本轮最终收尾（六色布局与 Logo 候选）：独立 japread-preview.exe 启动检查通过（窗口标题正确、Responding=true），已关闭测试窗口，用户原有 JapRead 窗口保留。A 最终采用不透明底色的 book-companion-v2.png，B 为 reading-companion.png；对比页 assets/logo-concepts/preview.html 同时展示 64／48／32 px 效果，截图 .local/japread-logo-comparison.png。已关闭临时浏览器与开发服务，确认 1420／1421 无监听；未提交或推送 Git。

### 正式 Windows 图标（2026-10-03）

用户已选定大和抚子阅读伙伴：assets/logo-concepts/reading-companion-nadeshiko.png。通过 Tauri CLI 转换图标，更新 src-tauri/icons 的 ICO／PNG，tauri.conf.json 显式声明 bundle.icon；Tauri Windows 默认窗口图标从同一 ICO 生成，EXE 嵌入图标已提取并目视确认。ICO 含 16／24／32／48／64／256 px。npm run desktop:build 已通过，正式产物 target/debug/japread.exe 已更新，取代上一轮需单独打开预览 EXE 的临时安排。本轮未重新启动桌面应用进行任务栏目视验收。

已创建本机 C:/Users/DimFi/Desktop/JapRead.lnk，TargetPath 指向本仓库 target/debug/japread.exe，IconLocation 使用该 EXE 的第一个图标；读回验证一致。构建没有启动 1420／1421 服务。未提交或推送。

### 阅读反馈修复（2026-10-03）

桌面 main.rs 的 windows_subsystem 不再仅限 release，调试 EXE 也使用 Windows GUI；构建后直接读取 PE OptionalHeader 确认 Subsystem=2。原来的空控制台来自 debug 构建，不是另一个后端服务。正式 target/debug/japread.exe 已成功覆盖，桌面快捷方式继续指向它。

抽屉旧 align-self:start 被重置，并明确 height:100% 与 border-box；右侧采用固定标题及 flex 内部滚动区，900 ms 无滚动后隐藏滚动条。浏览器实测可视 507 px、内容 2043 px，滚动到底可达；停止后 scrollbar-color 为透明。划词按钮在抽屉打开时移到左侧以免被遮挡。

新增 llm_selection，共用两种协议请求与凭据。后端读取原句、校验 Unicode start/end、计算重复词 occurrence，提示只解读目标片段；模型输出必须精确匹配唯一目标，否则拒绝；预览不写库。前端支持检查编辑后保存，右侧添加注释复用语境标注存储，默认 learn=false，不新增数据库格式。仍限同一句内划选。

批量请求原本已独立使用 maxTokens 与 timeout，无累计上下文；新增每批 1／2／4／8 句选择、独立预算说明、最近批 token 用量。测试扩充至两种协议连续批次预算保持、划词成功／错误范围拒绝及 Unicode 重复词定位；npm test 共 13 项通过，前端／桌面构建及格式检查通过。

界面验收使用 .local/settings-ui 隔离书库与本机模拟服务，长解读滚动、LLM 划词预览保存、手动注释保存及默认学习关闭通过。截图 .local/reader-fix-scroll.png 和 .local/reader-fix-batches.png。浏览器无 warn/error。未调用用户真实模型、未访问桌面连接资料。本轮不宣称完成原生全界面验收或本地小模型准确率评测。清除了测试开发 LLM 配置，关闭临时浏览器、模拟服务与 API/Vite，1420／1421 无监听。未提交／推送。

### 多套 LLM 配置与临时解读（2026-10-03）

Settings 增加 llm-profiles 索引（仅当前 ID 和 ID 列表）及 llm-profile-{id} 独立凭据；名称和整套配置均在凭据管理器，每套仍受 2400 UTF-8 字节限制，最多 20 套。旧 llm 条目先复制到固定 legacy ID、写索引成功后移除原条目；下次读取不会重复迁移。public 返回无密钥的卡片数据，不写 SQLite／localStorage／WebDAV。保存空密钥只保留同一套旧密钥，新建不继承；地址改变时必须输入密钥或 clearKey。新增 llm_profile_save/activate/remove；旧 settings_save llm 保持兼容并更新当前配置。删除当前配置不自动启用另一套。

UI 支持空状态添加、弹窗编辑、卡片选择及当前模型测试；切换不发送请求，测试才发送一句。使用隔离开发凭据、本机模拟服务验证两卡片新增／切换／连接测试。新增真实 Windows 凭据测试涵盖 legacy 迁移幂等性、公开数据脱敏、独立密钥、更换地址保护、清除密钥和删除当前项；npm test 共 14 项通过。

用户最终要求未保存划词结果随选中失效自动丢弃，不固定置顶。readerBlankClicked 过滤按钮、role=button 标注、链接、抽屉和输入控件，避免划选过程中误关；正文普通点击取消选中并收回。chooseAnnotation 清除临时结果并滚动详情顶部；本章列表仅未选中且无临时结果时显示。请求使用 generation 标记拒绝迟到结果；已发送网络请求仍完成，但失效结果不显示／保存。用 3 秒延迟模拟请求验证空白点击后不复现，且展开回到列表。截图 .local/llm-profile-cards.png。

最终前端与正式 target/debug/japread.exe 构建通过。原生完整界面验收与真实多服务商切换仍由用户配置后验证；未读取桌面实际凭据或调用用户真实模型。未提交／推送。

本轮收尾：最终浏览器确认点击已有解读后临时预览数量为 0、详情视图本章列表数量为 0；无 warn/error。已删除测试开发配置并关闭 API/Vite／延迟模拟服务与临时页面，1420／1421 无监听。

### 配置后偶发点击失效排查（2026-10-03）

用户保留了原生故障窗口：左侧导航可操作，主内容无法点击，无处理提示，切换页面不能恢复。用户控制台查询返回三个首页按钮，但随后返回的按钮 HTML 没有 disabled 属性；尚未查明这一诊断差异，不能宣称原生根因已确定。

隔离浏览器重复编辑配置、保存／取消、切换分类时，复现内容正常绘制而命中检测跳过 `.preferences-content` 的现象；当时无打开弹窗、inert 或禁用按钮。`preferences.ts` 现在只对标题文字做淡入，取消旧动画并在结束时清理效果，不再动画整个交互容器。`SettingsPage.vue` 的配置 dialog 使用 Teleport 到 body，按 profileOpen 挂载，关闭与卸载时清理；异步 nextTick 后检查组件仍存活。

修订后六轮保存／取消与分类切换的点击及 elementFromPoint 检查通过；Esc 关闭后继续操作、首页内容按钮跳转书库、返回设置通过。截图 `.local/click-fix.png`。前端生产构建和 Windows 独立 EXE 编译通过；尚待用户原生窗口复验，不将浏览器结果等同于 WebView2 全面验收。

为保留用户故障现场，未关闭用户 JapRead 或覆盖被占用的正式 EXE。独立产物 `target/debug/japread-clickfix.exe` 使用同一桌面书库及 Windows 凭据；用户退出旧版后打开此文件即可复验。桌面快捷方式仍指向 `target/debug/japread.exe`，待用户关闭旧程序后再更新正式产物。未读取用户实际凭据、未调用真实模型，未提交／推送。

已清除本轮开发测试 LLM 配置，关闭临时浏览器和本轮 API／Vite；1420／1421 监听已释放，用户原有 JapRead 保持运行。

### 本机 Gemma 输出兼容与批次反馈（2026-10-03）

用户报告 Gemma 全文解析出现合并校验错误，DeepSeek 曾第一批成功、第二批停止。经授权，在进程内读取当前 desktop LLM 凭据，仅向 loopback 模型发送两句原创诊断日文；未打印或落盘连接地址／密钥，未更改保存的配置。旧提示词约 28 秒收到完整 Responses 输出，7 条全部将 kind 填为提示示例中的「词汇或语法或读音」，部分 occurrence 填为条目序号，确认该次不是超时。新提示词使用合法示例、三选一类型和不同 quote 各自计数的规则，同配置约 20 秒得到 5 条通过校验的解读。不据此宣称完整文章或所有小模型可靠。

`llm.rs` 将合并错误拆成条目序号 + quote / kind / meaning / occurrence / sentenceId 的具体校验原因，仍严格拒绝错位引用，不擅自猜测类型或保存部分失败批次。新增回归用例覆盖错误类型、全局编号、空值、空列表和整批拒绝。`examples/diagnose_local_llm.rs` 是显式 `--run` 的 loopback 诊断工具，默认只输出非敏感模型参数，不切换配置、不请求远端服务、不保存结果。

`AnalysisPanel.vue` 默认每批 2 句（保留 1/2/4/8 选项）；增加当前批次等待秒数、完成批次耗时／条数记录，空列表有独立说明，异常包含失败批次和前批保留提示。关闭预览仍会丢弃未保存结果；继续在本次窗口内重试失败批次，不持久化后台队列。

目视验收还发现错误提示位于滚动区靠下时，会被 sticky 保存栏遮住；新增错误时自动滚动到居中位置（nearest 不足以避开覆盖栏），让失败原因直接可见。这是可复现的错误反馈问题，但没有证据能追溯用户 DeepSeek 当时的具体异常。

前端生产构建及 15 项 Rust 测试通过。隔离模拟前端验证三批流程：第1批成功，第2批模拟超时且第1批预览保留，继续只重试第2批；空结果计为已完成并有提示，随后第3批完成。截图 `.local/batch-failure.png`、`.local/batch-feedback-complete.png`。DeepSeek 原始失败响应没有记录，未发起新的远程模型请求，不能确认是超时／格式／输出截断；用户指定作品全文尚未复验。

独立 Windows 产物 `target/debug/japread-llmfix.exe` 包含本轮解析修复及前轮点击修复，使用原桌面书库与凭据；用户正在运行的正式 `japread.exe` 未关闭或覆盖，桌面快捷方式仍指向旧文件。未提交／推送。

最终居中报错目视检查通过，错误区域在保存栏上方完整可见。最终前端／独立 EXE 重建通过；临时诊断文章副本已移除，临时测试标签页和模拟 API／Vite 已关闭，1420／1421 无监听。用户原 JapRead 与本地模型服务保留，用户凭据未修改。

### Qwen 批量引用与 DeepSeek 未完成响应（2026-10-04）

本轮补测真实长度 UUID，复现 Qwen 将 UUID 字符抄错、短编号写成 S、2／S，以及漏字段和 occurrence 全局编号。此前以 s1/s2 测试两句不足以证明批量稳定。请求只发送 S1…S8，本地保留真实 ID 映射，启用严格 JSON 时枚举本批 ID；selection 同样使用短 ID，响应恢复真实 ID 后再严格比较划词范围。

`response_package` 对无效／缺失／null／数字 ID 只在 quote 在本批全部句子中恰好有一次精确匹配时修复；不能靠拼写相似、模糊匹配或条目顺序修复。有效 ID 仍限定其所属句子，quote 在所属句子只出现一次时由原文直接确定位置；多次出现必须给出有效 occurrence。代码测试覆盖跨句重复、句内重复、Unicode 和不匹配引用。

批量 `review_response` 独立验证每条，返回有效 package + warnings；警告包含跳过条目编号和原因，在前端批次记录中自动展开、完成状态中提示数量。不会把未通过校验条目交给保存接口。全批无效、破损 JSON／超数量仍报错；划词走严格单结果验证。此处取代上一轮“任何单条失败就丢弃整批”的策略，已有库记录不变。

Responses 区分 incomplete/max_output_tokens、content_filter、failed、queued/in_progress、cancelled、缺少 status；Chat 区分 length、content_filter、tool_calls 和未知 finish_reason。只呈现已知状态及数字用量，不回显上游 error.message、地址或密钥。输出截断提示包含输出／推理 token 与本次配置上限和耗时，不再一律建议增加限额。

用户授权配置实测（配置读取到内存，未修改）：Qwen 无约束诊断两句成功 6 条；较长短文仍能出现坏字段。严格格式请求被当前本地服务接受，但真实文章测试仍有不合规字段和空列表，不能宣称服务可靠执行约束。隔离测试库 `.local/llm-regression.sqlite3` 通过原来源适配器导入青空文库官方 workId=000013《十本の針》（75 句，官方 https://www.aozora.gr.jp/cards/000879/files/13_14563.html）。取第 9–16 句复验：当前 Qwen 参数、未强制 structured，9.6 秒返回 9 条，校验保留 8 条并警告跳过 1 条；随后每批 2 句的一次请求遭本地服务 HTTP 500，未自动重试。不能据此宣称完整作品稳定或空列表是正确学习分析。

DeepSeek 当前配置的同一真实第二批测试 18.3 秒完成、26 条有效解读，输出 4244 tokens（推理 2735），未重现用户原先约 30 秒失败。受控诊断将单次请求输出上限降到 128（保存值不变），复现 status=incomplete / reason=max_output_tokens，128 token 全用于推理；这证实诊断分支，但不能反推历史失败原因。没有自动切换用户当前配置、重试付费请求或提高保存的限额。Gemma 当前无对应保存配置，未重测。

诊断 example 新增 --list / --model 筛选已保存 LLM 配置，远端调用必须显式 --allow-remote；--limit、--structured 仅本次内存覆盖；--work / --size / --offset 从隔离库取正文，不读写用户实际笔记。输出不含密钥、地址或完整响应。19 项 Rust 测试通过，前端构建和 `target/release/japread-model-fix.exe` Release 构建通过。用户正在运行 target/release/japread.exe，未关闭或覆盖；新产物包含此前点击与模型修复。未提交／推送。

浏览器模拟验收通过：三批继续完成，警告所在记录自动展开，完成摘要显示跳过 1 条，无效条目不出现在 2 条可保存预览中；空列表单独说明。截图 `.local/model-review-warnings.png`。临时页面和本次模拟 API／Vite 已关闭，1420／1421 无监听，用户原有应用与模型服务保留。

### 阅读与按需解读验收准备（2026-10-04）

用户要求清理《十本の針》的手帐，暂缓全文／章节深度解析，调整注音按钮对齐。已移除阅读器的“解析章节”入口及 AnalysisPanel 挂载，保留组件和后端待 TODO 重新评估；设置说明和 README 改为划选词句 → LLM 解读 → 保留笔记流程。阅读工具栏与正文共同使用 18px 横向边距，1000px 以下共同使用 22px；浏览器实测注音按钮和正文右边缘均为 1203px，差值 0，注音开关状态切换正常。截图 `.local/reader-on-demand.png` 使用隔离模拟内容。

发现当前打包的 Codex 进程访问普通 AppData 路径可能受到 Windows 文件系统虚拟化影响，读到空数据库。实际桌面应用库通过扩展路径 `\\?\C:\Users\DimFi\AppData\Roaming\com.japread.desktop\japread.sqlite3` 定位；普通路径与 Packages/OpenAI.Codex 的 LocalCache 副本不能当作实际用户库。后续数据维护先核实作品数量和标题。

在用户授权范围内，SQLite backup 完整备份至 `.local/backups/japread-before-note-reset-20261004-004451.sqlite3`，完整性检查通过。事务中仅删除作品 ID `7cc27fef-8f13-4d00-86ba-969412ab28fe`（标题《十本の針》，sourceKey `aozora:000013`）的 37 条 annotations，剩余 0；核对正文、阅读进度及其他作品笔记未变化。凭据配置未读取或修改。

前端生产构建与正式 `cargo build -p japread --release --features custom-protocol` 通过。用户本轮没有运行 JapRead，已更新 `target/release/japread.exe`；桌面 JapRead 快捷方式确认指向该文件，可直接启动。验收标签页及本轮模拟 API／Vite 已关闭，1420／1421 无监听。未提交／推送。
