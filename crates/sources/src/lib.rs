use encoding_rs::Encoding;
#[cfg(all(test, windows))]
mod integration_tests;
mod llm;
mod settings;
mod webdav;
use scraper::{Html, Node, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
const CATALOG_URL: &str = "https://www.aozora.gr.jp/index_pages/list_person_all_extended_utf8.zip";
const TTL: u64 = 86400;

// Both desktop IPC and the development HTTP adapter share this boundary.
// Network requests never hold the SQLite mutex.
#[derive(Clone)]
pub struct Application {
    db: Arc<Mutex<japread_core::Store>>,
    sources: Arc<Mutex<Aozora>>,
    settings: Arc<Mutex<settings::Settings>>,
    llm_request: Arc<Mutex<()>>,
    dav_request: Arc<Mutex<()>>,
}
impl Application {
    pub fn open(db: &Path, cache: &Path, namespace: &str) -> Result<Self> {
        Ok(Self {
            db: Arc::new(Mutex::new(japread_core::Store::open(db)?)),
            sources: Arc::new(Mutex::new(Aozora::new(cache)?)),
            settings: Arc::new(Mutex::new(settings::Settings::new(namespace))),
            llm_request: Arc::new(Mutex::new(())),
            dav_request: Arc::new(Mutex::new(())),
        })
    }
    pub fn dispatch(&self, op: &str, payload: Value) -> Result<Value> {
        match op {
            "settings_get" => return self.settings.lock().map_err(err)?.public(),
            "llm_profile_save" => {
                return self.settings.lock().map_err(err)?.save_profile(
                    payload["id"].as_str(),
                    payload["name"].as_str().ok_or("缺少配置名称")?,
                    payload["config"].clone(),
                    payload["clearKey"].as_bool().unwrap_or(false),
                )
            }
            "llm_profile_activate" => {
                return self
                    .settings
                    .lock()
                    .map_err(err)?
                    .activate_profile(payload["id"].as_str().ok_or("缺少配置 ID")?)
            }
            "llm_profile_remove" => {
                return self
                    .settings
                    .lock()
                    .map_err(err)?
                    .remove_profile(payload["id"].as_str().ok_or("缺少配置 ID")?)
            }
            "settings_save" => {
                return self.settings.lock().map_err(err)?.save(
                    payload["section"].as_str().ok_or("缺少设置分区")?,
                    payload["config"].clone(),
                )
            }
            "settings_clear" => {
                let settings = self.settings.lock().map_err(err)?;
                settings.clear(payload["section"].as_str().ok_or("缺少设置分区")?)?;
                return settings.public();
            }
            "llm_analyze" | "llm_selection" | "llm_test" => {
                let _guard = self
                    .llm_request
                    .try_lock()
                    .map_err(|_| "已有 LLM 请求进行中，请等待当前批次完成")?;
                let mut config = self.settings.lock().map_err(err)?.llm()?;
                if config.endpoint.is_empty() {
                    return Err("请先在设置 → LLM 解析中保存连接配置".into());
                }
                if op == "llm_test" {
                    config.max_tokens = config.max_tokens.min(2048);
                    let doc:japread_core::Document=serde_json::from_value(json!({"id":"connection-test","title":"连接测试","author":"","kind":"文章","url":"","version":1,"chapters":[]})).map_err(err)?;
                    let sentences = vec![japread_core::Sentence {
                        id: "test-sentence".into(),
                        text: "猫が眠っています。".into(),
                        paragraph: 0,
                        readings: vec![],
                    }];
                    let result = llm::analyze(config, &doc, &sentences)?;
                    return Ok(
                        json!({"message":"模型连接与解析格式校验通过，未写入书库。","usage":result["usage"]}),
                    );
                }
                let doc = self
                    .db
                    .lock()
                    .map_err(err)?
                    .document(payload["documentId"].as_str().ok_or("缺少作品 ID")?)?;
                let chapter = doc
                    .chapters
                    .iter()
                    .find(|c| Some(c.id.as_str()) == payload["chapterId"].as_str())
                    .ok_or("章节不存在")?;
                let requested: Vec<String> = serde_json::from_value(payload["sentenceIds"].clone())
                    .map_err(|_| "缺少句子 ID 列表")?;
                let sentences: Vec<_> = chapter
                    .sentences
                    .iter()
                    .filter(|s| requested.contains(&s.id))
                    .cloned()
                    .collect();
                if sentences.len() != requested.len() {
                    return Err("句子列表包含重复项或本章之外的句子".into());
                }
                let result = if op == "llm_selection" {
                    let target = serde_json::from_value(payload["target"].clone())
                        .map_err(|_| "缺少有效的划词范围")?;
                    llm::analyze_selection(config, &doc, &sentences, target)?
                } else {
                    llm::analyze(config, &doc, &sentences)?
                };
                let package = serde_json::from_value(result["package"].clone()).map_err(err)?;
                self.db.lock().map_err(err)?.validate_package(&package)?;
                return Ok(result);
            }
            "webdav_test" | "webdav_upload" | "webdav_list" | "webdav_restore" => {
                let _guard = self
                    .dav_request
                    .try_lock()
                    .map_err(|_| "WebDAV 操作正在进行")?;
                let config = self.settings.lock().map_err(err)?.dav()?;
                if config.endpoint.is_empty() {
                    return Err("请先保存 WebDAV 连接配置".into());
                }
                return match op {
                    "webdav_test" => webdav::test(&config),
                    "webdav_list" => webdav::list(&config),
                    "webdav_upload" => {
                        let snapshot = self
                            .db
                            .lock()
                            .map_err(err)?
                            .dispatch("snapshot", json!({}))?;
                        webdav::upload(&config, snapshot)
                    }
                    _ => {
                        let snapshot = webdav::download(
                            &config,
                            payload["filename"].as_str().ok_or("请选择备份")?,
                        )?;
                        self.db.lock().map_err(err)?.merge_snapshot(snapshot)
                    }
                };
            }
            _ => {}
        }
        if op == "source_providers" {
            return Ok(
                json!([{"id":"aozora","name":"青空文库","enabled":true},{"id":"rss","name":"RSS / Atom","enabled":false}]),
            );
        }
        if op == "source_import" {
            if payload["provider"] != "aozora" {
                return Err("此来源尚未接入".into());
            }
            let work_id = payload["workId"].as_str().ok_or("缺少作品 ID")?;
            let key = format!("aozora:{work_id}");
            if let Some(doc) = self.db.lock().map_err(err)?.source_document(&key)? {
                return Ok(json!({"document":doc,"existing":true}));
            }
            let input = self.sources.lock().map_err(err)?.import(work_id)?;
            let doc = self
                .db
                .lock()
                .map_err(err)?
                .dispatch("import_document", input)?;
            return Ok(json!({"document":doc,"existing":false}));
        }
        if op == "source_catalog" {
            if payload["provider"] != "aozora" {
                return Err("此来源尚未接入".into());
            }
            return self.sources.lock().map_err(err)?.catalog(payload);
        }
        self.db.lock().map_err(err)?.dispatch(op, payload)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Author {
    id: String,
    name: String,
    reading: String,
    search: String,
    count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Work {
    id: String,
    title: String,
    subtitle: String,
    authors: Vec<String>,
    author_ids: Vec<String>,
    category: String,
    ndc: String,
    orthography: String,
    url: String,
    html_url: String,
    encoding: String,
    updated_at: String,
    copyright: String,
}
#[derive(Default)]
struct Catalog {
    authors: BTreeMap<String, Author>,
    works: BTreeMap<String, Work>,
}
fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}
fn category(ndc: &str) -> &'static str {
    // NDC's Japanese literature subdivisions; leave unclassified/foreign works honest.
    if ndc.contains("913") {
        "小说"
    } else if ndc.contains("914") {
        "随笔／评论"
    } else if ndc.contains("911") {
        "诗歌"
    } else if ndc.contains("912") {
        "戏剧"
    } else {
        "其他"
    }
}
fn parse_catalog(bytes: &[u8]) -> Result<Catalog> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(err)?;
    let file = archive
        .by_name("list_person_all_extended_utf8.csv")
        .map_err(err)?;
    if file.size() > 40_000_000 {
        return Err("目录解压大小超出限制".into());
    }
    let mut reader = csv::Reader::from_reader(file.take(40_000_001));
    let headers = reader.headers().map_err(err)?.clone();
    for name in [
        "作品ID",
        "作品名",
        "人物ID",
        "役割フラグ",
        "XHTML/HTMLファイルURL",
    ] {
        if !headers.iter().any(|h| h == name) {
            return Err(format!("官方目录缺少字段：{name}"));
        }
    }
    let mut catalog = Catalog::default();
    for row in reader.records() {
        let row = row.map_err(err)?;
        let get = |name: &str| {
            headers
                .iter()
                .position(|h| h == name)
                .and_then(|i| row.get(i))
                .unwrap_or("")
                .to_owned()
        };
        let person_id = get("人物ID");
        let name = format!("{}{}", get("姓"), get("名"));
        let author = get("役割フラグ") == "著者";
        let work_id = get("作品ID");
        if !work_id.chars().all(|c| c.is_ascii_digit()) || work_id.is_empty() {
            continue;
        }
        let w = catalog.works.entry(work_id.clone()).or_insert_with(|| {
            let ndc = get("分類番号");
            Work {
                id: work_id,
                title: get("作品名"),
                subtitle: get("副題"),
                authors: vec![],
                author_ids: vec![],
                category: category(&ndc).into(),
                ndc,
                orthography: get("文字遣い種別"),
                url: get("図書カードURL"),
                html_url: get("XHTML/HTMLファイルURL").replace("http://", "https://"),
                encoding: get("XHTML/HTMLファイル符号化方式"),
                updated_at: get("XHTML/HTMLファイル最終更新日"),
                copyright: get("作品著作権フラグ"),
            }
        });
        if author && !w.author_ids.contains(&person_id) {
            w.authors.push(name.clone());
            w.author_ids.push(person_id.clone());
            let a = catalog.authors.entry(person_id.clone()).or_insert_with(|| {
                let reading = format!("{}{}", get("姓読み"), get("名読み"));
                Author {
                    id: person_id,
                    search: normalized(&format!(
                        "{name} {reading} {} {}",
                        get("姓ローマ字"),
                        get("名ローマ字")
                    )),
                    name,
                    reading,
                    count: 0,
                }
            });
            a.count += 1;
        }
    }
    if catalog.works.is_empty() {
        return Err("官方目录没有可识别作品".into());
    }
    Ok(catalog)
}
struct Aozora {
    dir: PathBuf,
    data: Option<Catalog>,
    loaded_at: u64,
    warning: String,
    last_catalog_check: Option<Instant>,
    last_request: Option<Instant>,
    cooldown: Option<Instant>,
}
fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn valid_url(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("www.aozora.gr.jp")
            && u.port_or_known_default() == Some(443)
            && u.username().is_empty()
            && u.password().is_none()
    })
}
impl Aozora {
    fn new(cache: &Path) -> Result<Self> {
        let dir = cache.join("aozora");
        std::fs::create_dir_all(&dir).map_err(err)?;
        Ok(Self {
            dir,
            data: None,
            loaded_at: 0,
            warning: String::new(),
            last_catalog_check: None,
            last_request: None,
            cooldown: None,
        })
    }
    fn fetch(&mut self, url: &str, limit: u64) -> Result<Vec<u8>> {
        if !valid_url(url) {
            return Err("来源地址不属于青空文库 HTTPS 站点".into());
        }
        if self.cooldown.is_some_and(|t| t > Instant::now()) {
            return Err("来源站点繁忙，请稍后再试（至少等待一分钟）".into());
        }
        if let Some(last) = self.last_request {
            if let Some(wait) = Duration::from_secs(2).checked_sub(last.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(45))
            .connect_timeout(Duration::from_secs(15))
            .user_agent("JapRead/0.1 (personal reading; selected works only)")
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(err)?;
        self.last_request = Some(Instant::now());
        let response = client
            .get(url)
            .send()
            .map_err(|e| format!("青空文库连接失败，可稍后重试：{e}"))?;
        if matches!(response.status().as_u16(), 429 | 503) {
            let seconds = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60)
                .clamp(60, 3600);
            self.cooldown = Some(Instant::now() + Duration::from_secs(seconds));
            return Err(format!("青空文库暂忙，请 {seconds} 秒后重试"));
        }
        if !response.status().is_success() {
            return Err(format!(
                "青空文库返回 HTTP {}，未自动重试",
                response.status()
            ));
        }
        if response.content_length().is_some_and(|n| n > limit) {
            return Err("来源文件过大".into());
        }
        let mut bytes = Vec::new();
        response
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(err)?;
        if bytes.len() as u64 > limit {
            return Err("来源文件过大".into());
        }
        Ok(bytes)
    }
    fn ensure_catalog(&mut self, refresh: bool) -> Result<()> {
        let path = self.dir.join("catalog.zip");
        let stamp = modified(&path);
        if refresh && now().saturating_sub(stamp) < 600 {
            self.warning = "目录刚更新过，十分钟内复用缓存，避免重复请求。".into();
        }
        let needs_network =
            now().saturating_sub(stamp) >= TTL || (refresh && now().saturating_sub(stamp) >= 600);
        if self.data.is_some()
            && (!needs_network
                || self
                    .last_catalog_check
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(600)))
        {
            return Ok(());
        }
        if needs_network {
            self.last_catalog_check = Some(Instant::now());
            match self.fetch(CATALOG_URL, 10_000_000).and_then(|bytes| {
                let parsed = parse_catalog(&bytes)?;
                let temp = path.with_extension("zip.tmp");
                std::fs::write(&temp, bytes).map_err(err)?;
                std::fs::rename(temp, &path).map_err(err)?;
                Ok(parsed)
            }) {
                Ok(data) => {
                    self.data = Some(data);
                    self.loaded_at = modified(&path);
                    self.warning.clear();
                    return Ok(());
                }
                Err(e) => {
                    self.warning = format!("{e}。正在使用已有目录缓存。");
                    if !path.exists() {
                        return Err(e);
                    }
                }
            }
        }
        self.data = Some(parse_catalog(&std::fs::read(&path).map_err(err)?)?);
        self.loaded_at = stamp;
        Ok(())
    }
    fn catalog(&mut self, p: Value) -> Result<Value> {
        self.ensure_catalog(p["refresh"].as_bool().unwrap_or(false))?;
        let data = self.data.as_ref().unwrap();
        let query = normalized(p["query"].as_str().unwrap_or(""));
        let author = p["authorId"].as_str().unwrap_or("");
        let work_query = normalized(p["workQuery"].as_str().unwrap_or(""));
        let categories = p["categories"].as_array();
        let orthographies = p["orthographies"].as_array();
        let authors: Vec<_> = data
            .authors
            .values()
            .filter(|a| query.is_empty() || a.search.contains(&query))
            .collect();
        let works: Vec<_> = data
            .works
            .values()
            .filter(|w| !author.is_empty() && w.author_ids.iter().any(|a| a == author))
            .filter(|w| categories.is_none_or(|a| a.iter().any(|v| v == &w.category)))
            .filter(|w| orthographies.is_none_or(|a| a.iter().any(|v| v == &w.orthography)))
            .filter(|w| {
                work_query.is_empty()
                    || normalized(&format!("{} {}", w.title, w.subtitle)).contains(&work_query)
            })
            .collect();
        let offset = p["offset"].as_u64().unwrap_or(0).min(100000) as usize;
        Ok(
            json!({"authors":authors.iter().take(100).collect::<Vec<_>>(),"authorTotal":authors.len(),
            "works":works.iter().skip(offset).take(60).collect::<Vec<_>>(),"workTotal":works.len(),
            "cachedAt":self.loaded_at,"warning":self.warning,"categories":["小说","随笔／评论","诗歌","戏剧","其他"],
            "orthographies":["新字新仮名","新字旧仮名","旧字旧仮名","旧字新仮名","その他"]}),
        )
    }
    fn import(&mut self, work_id: &str) -> Result<Value> {
        self.ensure_catalog(false)?;
        let work = self
            .data
            .as_ref()
            .unwrap()
            .works
            .get(work_id)
            .ok_or("作品不在官方目录中")?
            .clone();
        if work.html_url.is_empty() {
            return Err("此作品未提供 XHTML 正文，请使用手动导入".into());
        }
        let path = self.dir.join(format!("{}.html", work.id));
        // Selected full texts are immutable local snapshots, reused for retries/offline reading.
        let bytes = if path.exists() {
            std::fs::read(&path).map_err(err)?
        } else {
            self.fetch(&work.html_url, 10_000_000)?
        };
        let encoding = Encoding::for_label(work.encoding.as_bytes())
            .or_else(|| {
                if work.encoding == "ShiftJIS" {
                    Some(encoding_rs::SHIFT_JIS)
                } else {
                    None
                }
            })
            .ok_or("不支持此作品的字符编码")?;
        let (html, _, bad) = encoding.decode(&bytes);
        if bad {
            return Err("作品字符编码异常，未保存，请改用手动导入".into());
        }
        let (chapters, gaiji) = parse_html(&html)?;
        let page = Html::parse_document(&html);
        let credits = page
            .select(&Selector::parse(".bibliographical_information").unwrap())
            .map(|e| e.text().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        if !path.exists() {
            std::fs::write(&path, &bytes).map_err(err)?;
        }
        Ok(
            json!({"title":work.title,"author":work.authors.join("、"),"kind":if work.category=="小说" {"小说"} else {"文章"},"url":work.url,"chapters":chapters,
            "source":{"key":format!("aozora:{}",work.id),"provider":"aozora","workId":work.id,"ndc":work.ndc,"orthography":work.orthography,"htmlUrl":work.html_url,"updatedAt":work.updated_at,"importedAt":now(),"copyright":work.copyright,"credits":credits,
            "notice":if gaiji>0 {format!("本书含 {gaiji} 处图片字／插图，以原站文字说明保留；可对照来源查看。原有注音不计入学习笔记。")} else {"保留原书注音；未执行 LLM 解析。".into()}}}),
        )
    }
}

#[derive(Default, Serialize)]
struct ParsedChapter {
    title: String,
    text: String,
    readings: Vec<japread_core::Reading>,
}
#[derive(Default)]
struct TextBuilder {
    chapters: Vec<ParsedChapter>,
    current: ParsedChapter,
    chars: usize,
    gaiji: usize,
}
impl TextBuilder {
    fn push(&mut self, s: &str) {
        self.current.text.push_str(s);
        self.chars += s.chars().count();
    }
    fn line(&mut self) {
        if !self.current.text.ends_with('\n') && !self.current.text.is_empty() {
            self.push("\n");
        }
    }
    fn finish(&mut self) {
        if !self.current.text.trim().is_empty() {
            let mut chapter = std::mem::take(&mut self.current);
            if chapter.title.is_empty() {
                chapter.title = "正文".into();
            }
            self.chapters.push(chapter);
        }
        self.chars = 0;
    }
    fn walk(&mut self, node: ego_tree::NodeRef<'_, Node>) {
        match node.value() {
            Node::Text(t) => {
                let text = t.text.replace(['\r', '\n', '\t'], "");
                self.push(&text);
            }
            Node::Element(e) => {
                let tag = e.name();
                if matches!(
                    tag,
                    "rt" | "rp" | "script" | "style" | "iframe" | "noscript"
                ) {
                    return;
                }
                if matches!(tag, "h2" | "h3" | "h4" | "h5") {
                    // Flat chapter list: retain headings in document order, without guessing volumes.
                    let mut heading = TextBuilder::default();
                    for child in node.children() {
                        heading.walk(child);
                    }
                    self.finish();
                    self.current = ParsedChapter {
                        title: heading.current.text.trim().into(),
                        ..Default::default()
                    };
                    return;
                }
                if tag == "br" {
                    self.line();
                    return;
                }
                if tag == "img" {
                    self.push(
                        e.attr("alt")
                            .filter(|s| !s.is_empty())
                            .unwrap_or("［原文插图］"),
                    );
                    self.gaiji += 1;
                    return;
                }
                if tag == "ruby" {
                    let start = self.chars;
                    for child in node.children() {
                        self.walk(child);
                    }
                    let mut reading = String::new();
                    for descendant in node.descendants() {
                        if let Some(element) = scraper::ElementRef::wrap(descendant) {
                            if element.value().name() == "rt" {
                                reading.push_str(&element.text().collect::<String>());
                            }
                        }
                    }
                    if self.chars > start && !reading.trim().is_empty() {
                        self.current.readings.push(japread_core::Reading {
                            start,
                            end: self.chars,
                            reading: reading.trim().into(),
                        });
                    }
                    return;
                }
                let block = matches!(tag, "p" | "div" | "li" | "blockquote" | "table" | "tr");
                if block {
                    self.line();
                }
                for child in node.children() {
                    self.walk(child);
                }
                if block {
                    self.line();
                }
            }
            _ => {}
        }
    }
}
fn parse_html(html: &str) -> Result<(Vec<ParsedChapter>, usize)> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(".main_text").unwrap();
    let root = document
        .select(&selector)
        .next()
        .ok_or("未找到青空文库正文区域，未导入页面杂项")?;
    let mut builder = TextBuilder::default();
    builder.walk(*root);
    builder.finish();
    if builder.chapters.is_empty() {
        return Err("作品正文为空".into());
    }
    Ok((builder.chapters, builder.gaiji))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_chapters_ruby_and_safe_text() {
        let (c,g)=parse_html(r#"<script>bad</script><div class="main_text"><h4>第一夜</h4>　猫<ruby><rb>😀漢</rb><rp>（</rp><rt>かん</rt><rp>）</rp></ruby>。<br><script>bad</script><h4>第二夜</h4><img alt="※外字">字。</div><div>底本</div>"#).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].title, "第一夜");
        assert_eq!(c[0].text.trim(), "猫😀漢。");
        assert_eq!(c[0].readings[0].start, 2);
        assert_eq!(c[0].readings[0].end, 4);
        assert_eq!(c[0].readings[0].reading, "かん");
        assert_eq!(g, 1);
        assert!(!c[1].text.contains("底本"));
        assert!(parse_html("<p>other page</p>").is_err());
    }
    #[test]
    fn official_csv_shape_deduplicates_and_searches_authors() {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "list_person_all_extended_utf8.csv",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all("作品ID,作品名,人物ID,姓,名,姓読み,名読み,役割フラグ,分類番号,XHTML/HTMLファイルURL\n000001,猫,000148,夏目,漱石,なつめ,そうせき,著者,NDC 913,https://www.aozora.gr.jp/a.html\n000001,猫,000148,夏目,漱石,なつめ,そうせき,著者,NDC 913,https://www.aozora.gr.jp/a.html\n000001,猫,000002,翻,訳,ほん,やく,翻訳者,NDC 913,https://www.aozora.gr.jp/a.html\n".as_bytes()).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let catalog = parse_catalog(&bytes).unwrap();
        assert_eq!(catalog.works.len(), 1);
        assert_eq!(catalog.authors.len(), 1);
        assert_eq!(catalog.authors["000148"].count, 1);
        assert!(catalog.authors["000148"]
            .search
            .contains(&normalized("なつめ そうせき")));
        assert_eq!(catalog.works["000001"].category, "小说");
        assert!(parse_catalog(b"not a zip").is_err());
    }
    #[test]
    fn source_host_is_fixed() {
        assert!(valid_url(CATALOG_URL));
        for url in [
            "http://www.aozora.gr.jp/a",
            "https://evil.test/a",
            "https://www.aozora.gr.jp.evil.test/a",
            "https://x@www.aozora.gr.jp/a",
            "https://www.aozora.gr.jp:444/a",
        ] {
            assert!(!valid_url(url));
        }
    }
}
