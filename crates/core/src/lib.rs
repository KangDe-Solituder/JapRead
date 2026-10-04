use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

type Result<T> = std::result::Result<T, String>;
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(err)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub start: usize,
    pub end: usize,
    pub reading: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sentence {
    pub id: String,
    pub text: String,
    pub paragraph: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readings: Vec<Reading>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub id: String,
    pub title: String,
    pub sentences: Vec<Sentence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub author: String,
    pub kind: String,
    pub url: String,
    pub version: u32,
    pub chapters: Vec<Chapter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Annotation {
    pub id: String,
    pub document_id: String,
    pub version: u32,
    pub sentence_id: String,
    pub start: usize,
    pub end: usize,
    pub quote: String,
    pub kind: String,
    pub reading: String,
    pub meaning: String,
    pub explanation: String,
    pub learn: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParsePackage {
    pub schema_version: u32,
    pub document_id: String,
    pub version: u32,
    pub annotations: Vec<Annotation>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Import {
    title: String,
    #[serde(default)]
    author: String,
    kind: String,
    #[serde(default)]
    url: String,
    chapters: Vec<ChapterInput>,
    #[serde(default)]
    source: Option<Value>,
}
#[derive(Deserialize)]
struct ChapterInput {
    title: String,
    text: String,
    #[serde(default)]
    readings: Vec<Reading>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub document_id: String,
    pub chapter_id: String,
    pub sentence_id: String,
    #[serde(default)]
    pub updated_at: u64,
}

pub struct Store {
    db: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_connection(Connection::open(path).map_err(err)?)
    }
    fn from_connection(db: Connection) -> Result<Self> {
        let version: u32 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(err)?;
        if version > 1 {
            return Err("数据库由较新版本创建，请升级 JapRead".into());
        }
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(err)?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;
          CREATE TABLE IF NOT EXISTS documents(id TEXT PRIMARY KEY, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS annotations(id TEXT PRIMARY KEY, document_id TEXT NOT NULL REFERENCES documents(id), body TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS annotation_document ON annotations(document_id);
          CREATE TABLE IF NOT EXISTS progress(document_id TEXT PRIMARY KEY REFERENCES documents(id), body TEXT NOT NULL);
          PRAGMA user_version=1;").map_err(err)?;
        Ok(Self { db })
    }
    pub fn source_document(&self, key: &str) -> Result<Option<Document>> {
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM documents WHERE json_extract(body, '$.source.key')=? LIMIT 1",
                [key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        body.map(|b| serde_json::from_str(&b).map_err(err))
            .transpose()
    }
    pub fn document(&self, doc_id: &str) -> Result<Document> {
        let data: Option<String> = self
            .db
            .query_row("SELECT body FROM documents WHERE id=?", [doc_id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        serde_json::from_str(&data.ok_or("作品不存在")?).map_err(err)
    }
    fn all<T: serde::de::DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        // Table names come exclusively from the three constants in dispatch.
        let mut stmt = self
            .db
            .prepare(&format!("SELECT body FROM {table} ORDER BY rowid DESC"))
            .map_err(err)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    fn validate(doc: &Document, a: &Annotation) -> Result<()> {
        if a.document_id != doc.id || a.version != doc.version {
            return Err("正文版本或作品 ID 不匹配".into());
        }
        let sentence = doc
            .chapters
            .iter()
            .flat_map(|c| &c.sentences)
            .find(|s| s.id == a.sentence_id)
            .ok_or("句子 ID 不存在")?;
        let chars: Vec<char> = sentence.text.chars().collect();
        if a.start >= a.end
            || a.end > chars.len()
            || chars[a.start..a.end].iter().collect::<String>() != a.quote
        {
            return Err("标注范围与引用原文不一致（范围按 Unicode 字符计数）".into());
        }
        if !["词汇", "语法", "读音"].contains(&a.kind.as_str()) || a.meaning.trim().is_empty()
        {
            return Err("请填写标注类型和释义".into());
        }
        if a.quote.len() > 10000
            || a.meaning.len() > 20000
            || a.explanation.len() > 50000
            || a.reading.len() > 10000
        {
            return Err("标注字段过长".into());
        }
        Ok(())
    }
    pub fn validate_package(&self, package: &ParsePackage) -> Result<()> {
        if package.schema_version != 1 || package.annotations.len() > 10000 {
            return Err("解析包格式不受支持".into());
        }
        let doc = self.document(&package.document_id)?;
        if doc.version != package.version {
            return Err("解析包正文版本过期".into());
        }
        for a in &package.annotations {
            Self::validate(&doc, a)?;
        }
        Ok(())
    }
    pub fn merge_snapshot(&mut self, value: Value) -> Result<Value> {
        use std::collections::HashSet;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Snapshot {
            documents: Vec<Document>,
            annotations: Vec<Annotation>,
            progress: Vec<Progress>,
        }
        let incoming: Snapshot = decode(value)?;
        if incoming.documents.len() > 1000 || incoming.annotations.len() > 100000 {
            return Err("备份条目超出当前版本限制".into());
        }
        let mut ids = HashSet::new();
        let mut sentence_ids = HashSet::new();
        let mut chapter_ids = HashSet::new();
        let mut annotation_ids = HashSet::new();
        for d in &incoming.documents {
            if d.id.is_empty()
                || !ids.insert(d.id.clone())
                || d.title.trim().is_empty()
                || d.version == 0
                || !["文章", "新闻", "小说"].contains(&d.kind.as_str())
                || d.chapters.is_empty()
                || d.chapters.len() > 500
            {
                return Err("备份作品结构无效".into());
            }
            if !d.url.is_empty() {
                let u = url::Url::parse(&d.url).map_err(|_| "备份来源链接无效")?;
                if !["http", "https"].contains(&u.scheme()) {
                    return Err("备份来源链接无效".into());
                }
            }
            let mut bytes = 0;
            for c in &d.chapters {
                if c.id.is_empty() || !chapter_ids.insert(c.id.clone()) || c.sentences.is_empty() {
                    return Err("备份章节结构无效".into());
                }
                for s in &c.sentences {
                    if s.id.is_empty()
                        || !sentence_ids.insert(s.id.clone())
                        || s.text.trim().is_empty()
                    {
                        return Err("备份句子结构无效".into());
                    }
                    bytes += s.text.len();
                    let len = s.text.chars().count();
                    let mut end = 0;
                    for r in &s.readings {
                        if r.start < end || r.start >= r.end || r.end > len {
                            return Err("备份注音范围无效".into());
                        }
                        end = r.end;
                    }
                }
            }
            if bytes > 5_000_000 {
                return Err("备份单部作品超出大小限制".into());
            }
        }
        for a in &incoming.annotations {
            if a.id.is_empty() || !annotation_ids.insert(a.id.clone()) {
                return Err("备份标注 ID 无效".into());
            }
            let doc = incoming
                .documents
                .iter()
                .find(|d| d.id == a.document_id)
                .ok_or("备份标注缺少作品")?;
            Self::validate(doc, a)?;
        }
        for p in &incoming.progress {
            let doc = incoming
                .documents
                .iter()
                .find(|d| d.id == p.document_id)
                .ok_or("备份进度缺少作品")?;
            if !doc
                .chapters
                .iter()
                .any(|c| c.id == p.chapter_id && c.sentences.iter().any(|s| s.id == p.sentence_id))
            {
                return Err("备份进度定位无效".into());
            }
        }
        let mut existing = self.all::<Document>("documents")?;
        let mut existing_notes = self.all::<Annotation>("annotations")?;
        let mut accepted = HashSet::new();
        let mut documents = 0;
        let mut annotations = 0;
        let mut conflicts = 0;
        let tx = self.db.transaction().map_err(err)?;
        for d in &incoming.documents {
            if let Some(old) = existing.iter().find(|e| e.id == d.id) {
                if old.version == d.version && json!(old.chapters) == json!(d.chapters) {
                    accepted.insert(d.id.clone());
                } else {
                    conflicts += 1;
                }
                continue;
            }
            if d.source
                .as_ref()
                .and_then(|s| s["key"].as_str())
                .is_some_and(|key| {
                    existing
                        .iter()
                        .any(|e| e.source.as_ref().and_then(|s| s["key"].as_str()) == Some(key))
                })
            {
                conflicts += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO documents(id,body) VALUES(?,?)",
                params![d.id, serde_json::to_string(d).map_err(err)?],
            )
            .map_err(err)?;
            accepted.insert(d.id.clone());
            existing.push(d.clone());
            documents += 1;
        }
        for a in incoming.annotations {
            if !accepted.contains(&a.document_id) {
                continue;
            }
            if existing_notes.iter().any(|e| {
                e.id == a.id
                    || (e.document_id == a.document_id
                        && e.sentence_id == a.sentence_id
                        && e.start == a.start
                        && e.end == a.end
                        && e.kind == a.kind)
            }) {
                continue;
            }
            Self::put_annotation(&tx, &a)?;
            existing_notes.push(a);
            annotations += 1;
        }
        for p in incoming.progress {
            if accepted.contains(&p.document_id) {
                tx.execute(
                    "INSERT OR IGNORE INTO progress(document_id,body) VALUES(?,?)",
                    params![p.document_id, serde_json::to_string(&p).map_err(err)?],
                )
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)?;
        Ok(json!({"documents":documents,"annotations":annotations,"conflicts":conflicts}))
    }
    fn put_annotation(tx: &rusqlite::Transaction<'_>, a: &Annotation) -> Result<()> {
        tx.execute("INSERT INTO annotations(id,document_id,body) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET body=excluded.body", params![a.id, a.document_id, serde_json::to_string(a).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn dispatch(&mut self, op: &str, payload: Value) -> Result<Value> {
        match op {
            "snapshot" => {
                let mut progress = self.all::<Progress>("progress")?;
                progress.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
                Ok(
                    json!({"documents": self.all::<Document>("documents")?, "annotations": self.all::<Annotation>("annotations")?, "progress": progress}),
                )
            }
            "import_document" => {
                let input: Import = decode(payload)?;
                if let Some(source) = &input.source {
                    let key = source["key"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or("来源缺少稳定 ID")?;
                    if let Some(doc) = self.source_document(key)? {
                        return Ok(json!(doc));
                    }
                }
                for chapter in &input.chapters {
                    let len = chapter.text.chars().count();
                    let mut previous = 0;
                    for r in &chapter.readings {
                        if r.start < previous
                            || r.start >= r.end
                            || r.end > len
                            || r.reading.len() > 10000
                        {
                            return Err("原书注音范围无效".into());
                        }
                        previous = r.end;
                    }
                }
                if input.title.trim().is_empty()
                    || input.title.chars().count() > 200
                    || !["文章", "新闻", "小说"].contains(&input.kind.as_str())
                {
                    return Err("请填写标题并选择内容类型".into());
                }
                if !input.url.trim().is_empty() {
                    let url = url::Url::parse(input.url.trim()).map_err(|_| "来源链接无效")?;
                    if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() {
                        return Err("来源链接仅支持 HTTP / HTTPS".into());
                    }
                }
                if input.chapters.is_empty()
                    || input.chapters.len() > 500
                    || input.chapters.iter().any(|c| c.text.trim().is_empty())
                {
                    return Err("每章都需要正文，最多 500 章".into());
                }
                if input.chapters.iter().map(|c| c.text.len()).sum::<usize>() > 5_000_000 {
                    return Err("本版本每次导入最多 5 MB 文本".into());
                }
                let chapters = input
                    .chapters
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| Chapter {
                        id: id(),
                        title: if c.title.trim().is_empty() {
                            format!("第 {} 章", i + 1)
                        } else {
                            c.title.trim().into()
                        },
                        sentences: split_with_readings(&c.text, &c.readings),
                    })
                    .collect();
                let doc = Document {
                    id: id(),
                    title: input.title.trim().into(),
                    author: input.author.trim().into(),
                    kind: input.kind,
                    url: input.url.trim().into(),
                    version: 1,
                    chapters,
                    source: input.source,
                };
                self.db
                    .execute(
                        "INSERT INTO documents(id,body) VALUES(?,?)",
                        params![doc.id, serde_json::to_string(&doc).map_err(err)?],
                    )
                    .map_err(err)?;
                Ok(json!(doc))
            }
            "save_annotation" => {
                let mut a: Annotation = decode(payload)?;
                Self::validate(&self.document(&a.document_id)?, &a)?;
                if a.id.is_empty() {
                    a.id = id();
                } else {
                    let old: Option<String> = self
                        .db
                        .query_row(
                            "SELECT document_id FROM annotations WHERE id=?",
                            [&a.id],
                            |r| r.get(0),
                        )
                        .optional()
                        .map_err(err)?;
                    if old.as_deref() != Some(&a.document_id) {
                        return Err("待编辑标注不存在或属于其他作品".into());
                    }
                }
                let tx = self.db.transaction().map_err(err)?;
                Self::put_annotation(&tx, &a)?;
                tx.commit().map_err(err)?;
                Ok(json!(a))
            }
            "set_learning" => {
                let a_id = payload["id"].as_str().ok_or("缺少标注 ID")?;
                let learn = payload["learn"].as_bool().ok_or("缺少学习状态")?;
                let body: String = self
                    .db
                    .query_row("SELECT body FROM annotations WHERE id=?", [a_id], |r| {
                        r.get(0)
                    })
                    .map_err(err)?;
                let mut a: Annotation = serde_json::from_str(&body).map_err(err)?;
                a.learn = learn;
                self.db
                    .execute(
                        "UPDATE annotations SET body=? WHERE id=?",
                        params![serde_json::to_string(&a).map_err(err)?, a_id],
                    )
                    .map_err(err)?;
                Ok(json!(a))
            }
            "import_annotations" => {
                let package: ParsePackage = decode(payload)?;
                if package.schema_version != 1
                    || package.annotations.is_empty()
                    || package.annotations.len() > 10000
                {
                    return Err("需要 schemaVersion=1 和 1～10000 条标注".into());
                }
                let doc = self.document(&package.document_id)?;
                if doc.version != package.version {
                    return Err("解析包正文版本过期".into());
                }
                for a in &package.annotations {
                    Self::validate(&doc, a)?;
                }
                let mut existing = self.all::<Annotation>("annotations")?;
                let tx = self.db.transaction().map_err(err)?;
                let mut added = 0;
                for mut a in package.annotations {
                    // Reimport must not overwrite a manually edited explanation or learning choice.
                    if existing.iter().any(|e| {
                        e.document_id == a.document_id
                            && e.sentence_id == a.sentence_id
                            && e.start == a.start
                            && e.end == a.end
                            && e.kind == a.kind
                    }) {
                        continue;
                    }
                    a.id = id();
                    a.learn = true;
                    Self::put_annotation(&tx, &a)?;
                    existing.push(a);
                    added += 1;
                }
                tx.commit().map_err(err)?;
                Ok(json!({"added":added}))
            }
            "save_progress" => {
                let mut p: Progress = decode(payload)?;
                p.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(err)?
                    .as_millis() as u64;
                let doc = self.document(&p.document_id)?;
                if !doc.chapters.iter().any(|c| {
                    c.id == p.chapter_id && c.sentences.iter().any(|s| s.id == p.sentence_id)
                }) {
                    return Err("阅读位置不属于该章节".into());
                }
                self.db.execute("INSERT INTO progress(document_id,body) VALUES(?,?) ON CONFLICT(document_id) DO UPDATE SET body=excluded.body", params![p.document_id, serde_json::to_string(&p).map_err(err)?]).map_err(err)?;
                Ok(json!(p))
            }
            _ => Err("不支持的操作".into()),
        }
    }
}

// Sentence text remains immutable within a document version. Offsets count Rust chars
// (Unicode scalar values), matching Array.from(text) on the frontend, not UTF-16 units.
fn split_sentences(text: &str) -> Vec<Sentence> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::new();
    for (paragraph, line) in normalized
        .split('\n')
        .filter(|l| !l.trim().is_empty())
        .enumerate()
    {
        let mut current = String::new();
        let mut terminated = false;
        for c in line.chars() {
            if terminated && !"」』）】\"'！？!?。".contains(c) {
                out.push(Sentence {
                    id: id(),
                    text: std::mem::take(&mut current),
                    paragraph,
                    readings: Vec::new(),
                });
                terminated = false;
            }
            current.push(c);
            if "。！？!?".contains(c) {
                terminated = true;
            }
        }
        if !current.is_empty() {
            out.push(Sentence {
                id: id(),
                text: current,
                paragraph,
                readings: Vec::new(),
            });
        }
    }
    out
}

fn split_with_readings(text: &str, readings: &[Reading]) -> Vec<Sentence> {
    let mut sentences = split_sentences(text);
    let mut cursor = 0;
    for sentence in &mut sentences {
        // Source import uses LF; reject mismatched positions instead of shifting anchors.
        if let Some(relative) = text[cursor..].find(&sentence.text) {
            let byte_start = cursor + relative;
            let start = text[..byte_start].chars().count();
            let end = start + sentence.text.chars().count();
            sentence.readings = readings
                .iter()
                .filter(|r| r.start >= start && r.end <= end)
                .map(|r| Reading {
                    start: r.start - start,
                    end: r.end - start,
                    reading: r.reading.clone(),
                })
                .collect();
            cursor = byte_start + sentence.text.len();
        }
    }
    sentences
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(s: &mut Store) -> Document {
        decode(s.dispatch("import_document",json!({"title":"試験", "kind":"小说", "chapters":[{"title":"一","text":"猫と猫😀。次の文。"},{"title":"二","text":"「本です。」彼は言う。"}]})).unwrap()).unwrap()
    }
    fn annotation(d: &Document) -> Annotation {
        Annotation {
            id: String::new(),
            document_id: d.id.clone(),
            version: 1,
            sentence_id: d.chapters[0].sentences[0].id.clone(),
            start: 2,
            end: 4,
            quote: "猫😀".into(),
            kind: "词汇".into(),
            reading: "ねこ".into(),
            meaning: "猫".into(),
            explanation: "第二次出现".into(),
            learn: true,
        }
    }
    #[test]
    fn source_dedup_and_native_reading_anchors() {
        let mut store = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let input = json!({"title":"原書", "kind":"小说", "source":{"key":"aozora:1"},
          "chapters":[{"title":"一", "text":"　猫。\n猫😀です。", "readings":[{"start":1,"end":2,"reading":"ねこ"},{"start":4,"end":5,"reading":"ねこ"}]}]});
        let doc: Document =
            decode(store.dispatch("import_document", input.clone()).unwrap()).unwrap();
        assert_eq!(doc.chapters[0].sentences[0].readings[0].start, 1);
        assert_eq!(doc.chapters[0].sentences[1].readings[0].start, 0);
        assert_eq!(doc.chapters[0].sentences[1].readings[0].end, 1);
        let duplicate: Document =
            decode(store.dispatch("import_document", input).unwrap()).unwrap();
        assert_eq!(duplicate.id, doc.id);
        assert_eq!(
            duplicate.chapters[0].sentences[0].id,
            doc.chapters[0].sentences[0].id
        );
        assert!(store.all::<Annotation>("annotations").unwrap().is_empty());
        assert_eq!(store.all::<Document>("documents").unwrap().len(), 1);
    }
    #[test]
    fn unicode_anchor_and_atomic_import() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut s);
        assert_eq!(d.chapters[1].sentences[0].text, "「本です。」");
        let a = annotation(&d);
        let mut bad = a.clone();
        bad.end = 8;
        let package = |items: Vec<Annotation>| json!({"schemaVersion":1,"documentId":d.id,"version":1,"annotations":items});
        assert!(s
            .dispatch("import_annotations", package(vec![a.clone(), bad]))
            .is_err());
        assert_eq!(s.all::<Annotation>("annotations").unwrap().len(), 0);
        assert_eq!(
            s.dispatch("import_annotations", package(vec![a.clone()]))
                .unwrap()["added"],
            1
        );
        let stored = s.all::<Annotation>("annotations").unwrap().remove(0);
        s.dispatch("set_learning", json!({"id":stored.id,"learn":false}))
            .unwrap();
        assert_eq!(
            s.dispatch("import_annotations", package(vec![a])).unwrap()["added"],
            0
        );
        assert!(!s.all::<Annotation>("annotations").unwrap()[0].learn);
    }
    #[test]
    fn reopen_preserves_data_and_rejects_wrong_positions() {
        let path = std::env::temp_dir().join(format!("japread-test-{}.sqlite3", id()));
        let doc_id;
        {
            let mut s = Store::open(&path).unwrap();
            let d = fixture(&mut s);
            doc_id = d.id.clone();
            let mut a = annotation(&d);
            a.version = 2;
            assert!(s.dispatch("save_annotation", json!(a)).is_err());
            let mut a = annotation(&d);
            a.quote = "猫".into();
            assert!(s.dispatch("save_annotation", json!(a)).is_err());
            s.dispatch("save_annotation", json!(annotation(&d)))
                .unwrap();
            assert!(s.dispatch("save_progress",json!({"documentId":d.id,"chapterId":d.chapters[1].id,"sentenceId":d.chapters[0].sentences[0].id})).is_err());
            s.dispatch("save_progress",json!({"documentId":d.id,"chapterId":d.chapters[0].id,"sentenceId":d.chapters[0].sentences[1].id})).unwrap();
        }
        {
            let s = Store::open(&path).unwrap();
            assert_eq!(s.document(&doc_id).unwrap().chapters.len(), 2);
            assert_eq!(s.all::<Annotation>("annotations").unwrap().len(), 1);
            assert_eq!(s.all::<Progress>("progress").unwrap().len(), 1);
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn backup_merge_is_atomic_and_preserves_local_edits() {
        let mut remote = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut remote);
        remote
            .dispatch("save_annotation", json!(annotation(&d)))
            .unwrap();
        remote.dispatch("save_progress",json!({"documentId":d.id,"chapterId":d.chapters[0].id,"sentenceId":d.chapters[0].sentences[0].id})).unwrap();
        let backup = remote.dispatch("snapshot", json!({})).unwrap();
        let mut local = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let mut broken = backup.clone();
        broken["annotations"][0]["quote"] = json!("不存在");
        assert!(local.merge_snapshot(broken).is_err());
        assert!(local.all::<Document>("documents").unwrap().is_empty());
        assert_eq!(
            local.merge_snapshot(backup.clone()).unwrap()["documents"],
            1
        );
        let mut edited = local.all::<Annotation>("annotations").unwrap().remove(0);
        edited.meaning = "手工修订".into();
        edited.learn = false;
        local.dispatch("save_annotation", json!(edited)).unwrap();
        local.dispatch("save_progress",json!({"documentId":d.id,"chapterId":d.chapters[1].id,"sentenceId":d.chapters[1].sentences[0].id})).unwrap();
        assert_eq!(
            local.merge_snapshot(backup.clone()).unwrap()["annotations"],
            0
        );
        assert_eq!(
            local.all::<Annotation>("annotations").unwrap()[0].meaning,
            "手工修订"
        );
        assert!(!local.all::<Annotation>("annotations").unwrap()[0].learn);
        assert_eq!(
            local.all::<Progress>("progress").unwrap()[0].chapter_id,
            d.chapters[1].id
        );
        let mut conflict = backup;
        conflict["documents"][0]["version"] = json!(2);
        conflict["annotations"][0]["version"] = json!(2);
        assert_eq!(local.merge_snapshot(conflict).unwrap()["conflicts"], 1);
        assert_eq!(local.document(&d.id).unwrap().version, 1);
    }
    #[test]
    fn source_validation() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        for url in ["javascript:alert(1)", "file:///C:/foo", "bad url"] {
            assert!(s.dispatch("import_document",json!({"title":"x","kind":"文章","url":url,"chapters":[{"title":"正文","text":"x"}]})).is_err());
        }
    }
}
