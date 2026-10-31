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
pub struct ChapterAnalysis {
    pub main_idea: String,
    pub overview: String,
    pub key_points: Vec<String>,
    pub model: String,
    pub updated_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedReading {
    pub sentence_id: String,
    pub start: usize,
    pub end: usize,
    pub quote: String,
    pub reading: String,
}
pub fn is_kanji(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x323af)
        || c == '々'
}
pub fn validate_readings(chapter: &Chapter, items: &[GeneratedReading]) -> Result<()> {
    if items.len() > 100000 {
        return Err("注音数量过多".into());
    }
    let mut occupied = std::collections::HashSet::new();
    for r in items {
        let s = chapter
            .sentences
            .iter()
            .find(|s| s.id == r.sentence_id)
            .ok_or("注音句子不存在")?;
        let chars: Vec<_> = s.text.chars().collect();
        if r.start >= r.end
            || r.end > chars.len()
            || chars[r.start..r.end].iter().collect::<String>() != r.quote
            || !r.quote.chars().any(is_kanji)
            || r.reading.is_empty()
            || r.reading.chars().count() > 200
            || !r
                .reading
                .chars()
                .all(|c| matches!(c as u32, 0x3041..=0x3096 | 0x30a1..=0x30fa) || c == 'ー')
            || s.readings
                .iter()
                .any(|n| n.start < r.end && n.end > r.start)
            || (r.start..r.end).any(|i| !occupied.insert((&r.sentence_id, i)))
        {
            return Err("注音引用、假名或范围无效，或与原书／其他注音重叠".into());
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated_readings: Vec<GeneratedReading>,
    pub id: String,
    pub title: String,
    pub sentences: Vec<Sentence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<ChapterAnalysis>,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterAnalysisResult {
    pub chapter_id: String,
    pub analysis: ChapterAnalysis,
    pub package: ParsePackage,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisTask {
    pub id: String,
    pub document_id: String,
    pub document_version: u32,
    pub chapter_ids: Vec<String>,
    #[serde(default)]
    pub chapter_results: Vec<ChapterAnalysisResult>,
    #[serde(default)]
    pub saved_chapter_ids: Vec<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
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
          CREATE TRIGGER IF NOT EXISTS documents_source_key_unique
            BEFORE INSERT ON documents FOR EACH ROW
            WHEN json_extract(NEW.body, '$.source.key') IS NOT NULL
              AND EXISTS(SELECT 1 FROM documents
                WHERE json_extract(body, '$.source.key')=json_extract(NEW.body, '$.source.key'))
            BEGIN SELECT RAISE(ABORT, 'duplicate source key'); END;
          CREATE TABLE IF NOT EXISTS annotations(id TEXT PRIMARY KEY, document_id TEXT NOT NULL REFERENCES documents(id), body TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS annotation_document ON annotations(document_id);
          CREATE TABLE IF NOT EXISTS progress(document_id TEXT PRIMARY KEY REFERENCES documents(id), body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS analysis_tasks(id TEXT PRIMARY KEY, document_id TEXT NOT NULL REFERENCES documents(id), body TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS analysis_task_document ON analysis_tasks(document_id);
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
    fn now() -> Result<u64> {
        Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_millis() as u64)
    }
    fn update_task(&self, task: &AnalysisTask) -> Result<AnalysisTask> {
        let changed = self
            .db
            .execute(
                "UPDATE analysis_tasks SET body=? WHERE id=?",
                params![serde_json::to_string(task).map_err(err)?, task.id],
            )
            .map_err(err)?;
        if changed != 1 {
            return Err("分析任务不存在".into());
        }
        Ok(task.clone())
    }
    pub fn create_task(
        &mut self,
        document_id: &str,
        document_version: u32,
        chapter_ids: Vec<String>,
    ) -> Result<AnalysisTask> {
        let doc = self.document(document_id)?;
        if doc.version != document_version {
            return Err("分析任务正文版本过期".into());
        }
        let mut unique = std::collections::HashSet::new();
        if chapter_ids.is_empty()
            || chapter_ids.iter().any(|chapter_id| {
                !unique.insert(chapter_id) || !doc.chapters.iter().any(|c| &c.id == chapter_id)
            })
        {
            return Err("分析任务章节无效或重复".into());
        }
        let now = Self::now()?;
        let task = AnalysisTask {
            id: id(),
            document_id: document_id.into(),
            document_version,
            chapter_ids,
            chapter_results: Vec::new(),
            saved_chapter_ids: Vec::new(),
            status: "queued".into(),
            error: None,
            created_at: now,
            updated_at: now,
        };
        self.db
            .execute(
                "INSERT INTO analysis_tasks(id,document_id,body) VALUES(?,?,?)",
                params![
                    task.id,
                    task.document_id,
                    serde_json::to_string(&task).map_err(err)?
                ],
            )
            .map_err(err)?;
        Ok(task)
    }
    pub fn task(&self, task_id: &str) -> Result<AnalysisTask> {
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM analysis_tasks WHERE id=?",
                [task_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        serde_json::from_str(&body.ok_or("分析任务不存在")?).map_err(err)
    }
    pub fn tasks_for_document(&self, document_id: &str) -> Result<Vec<AnalysisTask>> {
        let mut stmt = self
            .db
            .prepare("SELECT body FROM analysis_tasks WHERE document_id=? ORDER BY rowid DESC")
            .map_err(err)?;
        let rows = stmt
            .query_map([document_id], |r| r.get::<_, String>(0))
            .map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn mark_task_running(&mut self, task_id: &str) -> Result<AnalysisTask> {
        let mut task = self.task(task_id)?;
        if !["queued", "running"].contains(&task.status.as_str()) {
            return Err("只有排队中的分析任务可以开始运行".into());
        }
        task.status = "running".into();
        task.error = None;
        task.updated_at = Self::now()?;
        self.update_task(&task)
    }
    pub fn complete_chapter(
        &mut self,
        task_id: &str,
        result: ChapterAnalysisResult,
    ) -> Result<AnalysisTask> {
        let mut task = self.task(task_id)?;
        if !["running", "paused"].contains(&task.status.as_str()) {
            return Err("只有运行或暂停中的分析任务可以写入章节结果".into());
        }
        if !task.chapter_ids.contains(&result.chapter_id)
            || task
                .chapter_results
                .iter()
                .any(|r| r.chapter_id == result.chapter_id)
        {
            return Err("章节不属于该分析任务或已完成".into());
        }
        self.validate_package(&result.package)?;
        if result.package.document_id != task.document_id
            || result.package.version != task.document_version
        {
            return Err("章节结果与分析任务的作品或版本不匹配".into());
        }
        let doc = self.document(&task.document_id)?;
        let chapter = doc
            .chapters
            .iter()
            .find(|c| c.id == result.chapter_id)
            .ok_or("章节不存在")?;
        if result
            .package
            .annotations
            .iter()
            .any(|a| !chapter.sentences.iter().any(|s| s.id == a.sentence_id))
        {
            return Err("章节结果包含其他章节的标注".into());
        }
        task.chapter_results.push(result);
        if task.chapter_results.len() == task.chapter_ids.len() {
            task.status = "completed".into();
        }
        task.updated_at = Self::now()?;
        self.update_task(&task)
    }
    pub fn fail_task(&mut self, task_id: &str, error: &str) -> Result<AnalysisTask> {
        let mut task = self.task(task_id)?;
        if task.status == "saved" {
            return Err("已保存的分析任务不能修改为失败".into());
        }
        task.status = "failed".into();
        task.error = Some(error.into());
        task.updated_at = Self::now()?;
        self.update_task(&task)
    }
    pub fn pause_task(&mut self, task_id: &str) -> Result<AnalysisTask> {
        let mut task = self.task(task_id)?;
        if !["queued", "running", "paused"].contains(&task.status.as_str()) {
            return Err("当前分析任务状态不能暂停".into());
        }
        task.status = "paused".into();
        task.updated_at = Self::now()?;
        self.update_task(&task)
    }
    pub fn resume_task(&mut self, task_id: &str) -> Result<AnalysisTask> {
        let mut task = self.task(task_id)?;
        if !["paused", "failed"].contains(&task.status.as_str()) {
            return Err("只有已暂停或失败的分析任务可以恢复".into());
        }
        task.status = "queued".into();
        task.error = None;
        task.updated_at = Self::now()?;
        self.update_task(&task)
    }
    pub fn delete_task(&mut self, task_id: &str) -> Result<()> {
        if self
            .db
            .execute("DELETE FROM analysis_tasks WHERE id=?", [task_id])
            .map_err(err)?
            != 1
        {
            return Err("分析任务不存在".into());
        }
        Ok(())
    }
    pub fn recover_interrupted_tasks(&mut self) -> Result<usize> {
        let mut tasks = Vec::new();
        for value in self.all::<Value>("analysis_tasks")? {
            let legacy = value.get("savedChapterIds").is_none();
            let mut task: AnalysisTask = decode(value)?;
            if legacy {
                let doc = self.document(&task.document_id)?;
                // An older analysis is not evidence of saving this result.
                // Keep historical saved tasks terminal when a newer result has
                // since replaced them, rather than resurrecting old previews.
                for result in &task.chapter_results {
                    if doc.chapters.iter().any(|c| {
                        c.id == result.chapter_id
                            && c.analysis.as_ref().is_some_and(|a| {
                                json!(a) == json!(result.analysis)
                                    || (task.status == "saved"
                                        && a.updated_at > result.analysis.updated_at)
                            })
                    }) {
                        task.saved_chapter_ids.push(result.chapter_id.clone());
                    }
                }
                if task.status == "saved" && task.saved_chapter_ids.len() != task.chapter_ids.len()
                {
                    task.status = "completed".into();
                }
            }
            if task.status == "completed" && task.chapter_results.len() < task.chapter_ids.len() {
                task.status = "queued".into();
            }
            tasks.push((task, legacy));
        }
        let now = Self::now()?;
        let tx = self.db.transaction().map_err(err)?;
        let mut recovered = 0;
        for (task, legacy) in &mut tasks {
            let interrupted = ["queued", "running"].contains(&task.status.as_str());
            if interrupted {
                task.status = "paused".into();
                recovered += 1;
            }
            if interrupted || *legacy {
                task.updated_at = now;
                tx.execute(
                    "UPDATE analysis_tasks SET body=? WHERE id=?",
                    params![serde_json::to_string(task).map_err(err)?, task.id],
                )
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)?;
        Ok(recovered)
    }
    pub fn save_task_results(
        &mut self,
        task_id: &str,
        chapter_ids: &[String],
        package: &ParsePackage,
    ) -> Result<AnalysisTask> {
        let mut task = self.task(task_id)?;
        if task.status == "saved" {
            return Err("分析任务结果已保存".into());
        }
        let mut doc = self.document(&task.document_id)?;
        if doc.version != task.document_version
            || package.document_id != task.document_id
            || package.version != task.document_version
        {
            return Err("分析任务或解析包正文版本不匹配".into());
        }
        self.validate_package(package)?;
        let mut unique = std::collections::HashSet::new();
        if chapter_ids.is_empty()
            || chapter_ids.iter().any(|chapter_id| {
                !unique.insert(chapter_id) || !task.chapter_ids.contains(chapter_id)
            })
        {
            return Err("待保存章节无效或重复".into());
        }
        for chapter_id in chapter_ids {
            let result = task
                .chapter_results
                .iter()
                .find(|r| &r.chapter_id == chapter_id)
                .ok_or("待保存章节尚未完成")?;
            let chapter = doc
                .chapters
                .iter_mut()
                .find(|c| &c.id == chapter_id)
                .ok_or("待保存章节不存在")?;
            chapter.analysis = Some(result.analysis.clone());
        }
        if package.annotations.iter().any(|a| {
            !doc.chapters.iter().any(|c| {
                chapter_ids.contains(&c.id) && c.sentences.iter().any(|s| s.id == a.sentence_id)
            })
        }) {
            return Err("解析包包含未选章节的标注".into());
        }
        let mut existing = self.all::<Annotation>("annotations")?;
        let tx = self.db.transaction().map_err(err)?;
        tx.execute(
            "UPDATE documents SET body=? WHERE id=?",
            params![serde_json::to_string(&doc).map_err(err)?, doc.id],
        )
        .map_err(err)?;
        for mut a in package.annotations.iter().cloned() {
            if existing.iter().any(|e| {
                (!a.id.is_empty() && e.id == a.id)
                    || (e.document_id == a.document_id
                        && e.sentence_id == a.sentence_id
                        && e.start == a.start
                        && e.end == a.end
                        && e.kind == a.kind)
            }) {
                continue;
            }
            a.id = id();
            a.learn = true;
            Self::put_annotation(&tx, &a)?;
            existing.push(a);
        }
        for chapter_id in chapter_ids {
            if !task.saved_chapter_ids.contains(chapter_id) {
                task.saved_chapter_ids.push(chapter_id.clone());
            }
        }
        if task
            .chapter_ids
            .iter()
            .all(|id| task.saved_chapter_ids.contains(id))
        {
            task.status = "saved".into();
            task.error = None;
        }
        task.updated_at = Self::now()?;
        tx.execute(
            "UPDATE analysis_tasks SET body=? WHERE id=?",
            params![serde_json::to_string(&task).map_err(err)?, task.id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(task)
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
                validate_readings(c, &c.generated_readings)?;
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
        let chapter_content = |chapters: &[Chapter]| {
            json!(chapters
                .iter()
                .cloned()
                .map(|mut c| {
                    c.analysis = None;
                    c.generated_readings.clear();
                    c
                })
                .collect::<Vec<_>>())
        };
        let mut accepted = HashSet::new();
        let mut documents = 0;
        let mut annotations = 0;
        let mut conflicts = 0;
        let tx = self.db.transaction().map_err(err)?;
        for d in &incoming.documents {
            if let Some(old) = existing.iter().find(|e| e.id == d.id) {
                if old.version == d.version
                    && chapter_content(&old.chapters) == chapter_content(&d.chapters)
                {
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
            "readings_save" => {
                let mut doc = self.document(payload["documentId"].as_str().ok_or("缺少作品")?)?;
                if payload["version"].as_u64() != Some(doc.version as u64) {
                    return Err("正文版本已变更".into());
                }
                let chapter = doc
                    .chapters
                    .iter_mut()
                    .find(|c| Some(c.id.as_str()) == payload["chapterId"].as_str())
                    .ok_or("章节不存在")?;
                let items: Vec<GeneratedReading> = decode(payload["items"].clone())?;
                validate_readings(chapter, &items)?;
                let mut items = items;
                if let Some(scope) = payload.get("paragraphIds") {
                    let scope: Vec<usize> = decode(scope.clone())?;
                    if scope.is_empty()
                        || scope
                            .iter()
                            .any(|p| !chapter.sentences.iter().any(|s| s.paragraph == *p))
                        || items.iter().any(|r| {
                            !chapter
                                .sentences
                                .iter()
                                .any(|s| s.id == r.sentence_id && scope.contains(&s.paragraph))
                        })
                    {
                        return Err("注音超出所选段落".into());
                    }
                    items.extend(
                        chapter
                            .generated_readings
                            .iter()
                            .filter(|r| {
                                chapter
                                    .sentences
                                    .iter()
                                    .any(|s| s.id == r.sentence_id && !scope.contains(&s.paragraph))
                            })
                            .cloned(),
                    );
                }
                validate_readings(chapter, &items)?;
                chapter.generated_readings = items;
                self.db
                    .execute(
                        "UPDATE documents SET body=? WHERE id=?",
                        params![serde_json::to_string(&doc).map_err(err)?, doc.id],
                    )
                    .map_err(err)?;
                return Ok(json!({"saved": true}));
            }
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
                        generated_readings: vec![],
                        id: id(),
                        title: if c.title.trim().is_empty() {
                            format!("第 {} 章", i + 1)
                        } else {
                            c.title.trim().into()
                        },
                        sentences: split_with_readings(&c.text, &c.readings),
                        analysis: None,
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
    fn analysis(label: &str) -> ChapterAnalysis {
        ChapterAnalysis {
            main_idea: format!("{label}主旨"),
            overview: format!("{label}概览"),
            key_points: vec!["要点".into()],
            model: "test-model".into(),
            updated_at: 123,
        }
    }
    fn chapter_result(d: &Document, chapter: usize, label: &str) -> ChapterAnalysisResult {
        ChapterAnalysisResult {
            chapter_id: d.chapters[chapter].id.clone(),
            analysis: analysis(label),
            package: ParsePackage {
                schema_version: 1,
                document_id: d.id.clone(),
                version: d.version,
                annotations: Vec::new(),
            },
            warnings: vec!["测试警告".into()],
            elapsed_ms: 42,
            input_tokens: Some(10),
            output_tokens: Some(20),
        }
    }
    #[test]
    fn analysis_task_persists_chapters_and_recovers_interruption() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut s);
        assert!(s
            .create_task(&d.id, 2, vec![d.chapters[0].id.clone()])
            .is_err());
        let task = s
            .create_task(
                &d.id,
                d.version,
                vec![d.chapters[0].id.clone(), d.chapters[1].id.clone()],
            )
            .unwrap();
        assert_eq!(task.status, "queued");
        assert_eq!(json!(task)["documentVersion"], 1);
        s.mark_task_running(&task.id).unwrap();
        s.complete_chapter(&task.id, chapter_result(&d, 0, "一"))
            .unwrap();
        assert_eq!(s.recover_interrupted_tasks().unwrap(), 1);
        let paused = s.task(&task.id).unwrap();
        assert_eq!(paused.status, "paused");
        assert_eq!(paused.chapter_results.len(), 1);
        assert_eq!(paused.chapter_results[0].chapter_id, d.chapters[0].id);
        assert_eq!(s.resume_task(&task.id).unwrap().status, "queued");
        s.mark_task_running(&task.id).unwrap();
        s.pause_task(&task.id).unwrap();
        let completed = s
            .complete_chapter(&task.id, chapter_result(&d, 1, "二"))
            .unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.chapter_results.len(), 2);
        assert_eq!(s.tasks_for_document(&d.id).unwrap().len(), 1);

        let failed = s
            .create_task(&d.id, d.version, vec![d.chapters[0].id.clone()])
            .unwrap();
        assert_eq!(
            s.fail_task(&failed.id, "测试失败")
                .unwrap()
                .error
                .as_deref(),
            Some("测试失败")
        );
        assert_eq!(s.resume_task(&failed.id).unwrap().status, "queued");
        s.fail_task(&failed.id, "再次失败").unwrap();
        s.delete_task(&failed.id).unwrap();
        assert!(s.task(&failed.id).is_err());
    }
    #[test]
    fn save_task_results_is_atomic_and_preserves_existing_annotations() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut s);
        s.dispatch("save_annotation", json!(annotation(&d)))
            .unwrap();
        let existing = s.all::<Annotation>("annotations").unwrap().remove(0);
        s.dispatch("set_learning", json!({"id":existing.id,"learn":false}))
            .unwrap();

        let task = s
            .create_task(&d.id, d.version, vec![d.chapters[0].id.clone()])
            .unwrap();
        s.mark_task_running(&task.id).unwrap();
        s.complete_chapter(&task.id, chapter_result(&d, 0, "一"))
            .unwrap();
        let mut replacement = annotation(&d);
        replacement.explanation = "模型改写".into();
        let mut added = replacement.clone();
        added.start = 0;
        added.end = 2;
        added.quote = "猫と".into();
        added.explanation = "新增解释".into();
        let package = ParsePackage {
            schema_version: 1,
            document_id: d.id.clone(),
            version: d.version,
            annotations: vec![replacement, added],
        };
        let saved = s
            .save_task_results(&task.id, &[d.chapters[0].id.clone()], &package)
            .unwrap();
        assert_eq!(saved.status, "saved");
        assert_eq!(
            s.document(&d.id).unwrap().chapters[0]
                .analysis
                .as_ref()
                .unwrap()
                .main_idea,
            "一主旨"
        );
        let notes = s.all::<Annotation>("annotations").unwrap();
        assert_eq!(notes.len(), 2);
        let preserved = notes.iter().find(|a| a.id == existing.id).unwrap();
        assert_eq!(preserved.explanation, "第二次出现");
        assert!(!preserved.learn);
        assert!(s
            .save_task_results(&task.id, &[d.chapters[0].id.clone()], &package)
            .is_err());
        assert_eq!(s.all::<Annotation>("annotations").unwrap().len(), 2);

        let understanding_only = s
            .create_task(&d.id, d.version, vec![d.chapters[1].id.clone()])
            .unwrap();
        s.mark_task_running(&understanding_only.id).unwrap();
        s.complete_chapter(&understanding_only.id, chapter_result(&d, 1, "二"))
            .unwrap();
        let mut wrong_version = ParsePackage {
            schema_version: 1,
            document_id: d.id.clone(),
            version: 2,
            annotations: Vec::new(),
        };
        assert!(s
            .save_task_results(
                &understanding_only.id,
                &[d.chapters[1].id.clone()],
                &wrong_version,
            )
            .is_err());
        assert!(s.document(&d.id).unwrap().chapters[1].analysis.is_none());
        wrong_version.version = 1;
        assert_eq!(
            s.save_task_results(
                &understanding_only.id,
                &[d.chapters[1].id.clone()],
                &wrong_version,
            )
            .unwrap()
            .status,
            "saved"
        );
        assert!(s.document(&d.id).unwrap().chapters[1].analysis.is_some());
        assert!(s.save_task_results("missing", &[], &wrong_version).is_err());
    }
    #[test]
    fn partial_task_save_leaves_remaining_chapters_available() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut s);
        let chapter_ids = d.chapters.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let task = s
            .create_task(&d.id, d.version, chapter_ids.clone())
            .unwrap();
        s.mark_task_running(&task.id).unwrap();
        s.complete_chapter(&task.id, chapter_result(&d, 0, "一"))
            .unwrap();
        s.complete_chapter(&task.id, chapter_result(&d, 1, "二"))
            .unwrap();
        let package = ParsePackage {
            schema_version: 1,
            document_id: d.id.clone(),
            version: d.version,
            annotations: Vec::new(),
        };
        assert_eq!(
            s.save_task_results(&task.id, &[chapter_ids[0].clone()], &package)
                .unwrap()
                .status,
            "completed"
        );
        assert_eq!(
            s.save_task_results(&task.id, &[chapter_ids[1].clone()], &package)
                .unwrap()
                .status,
            "saved"
        );
    }
    #[test]
    fn partial_save_preserves_execution_and_failure_state() {
        for state in ["running", "paused", "failed"] {
            let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
            let d = fixture(&mut s);
            let ids = d.chapters.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
            let t = s.create_task(&d.id, d.version, ids.clone()).unwrap();
            s.mark_task_running(&t.id).unwrap();
            let first = chapter_result(&d, 0, "first");
            s.complete_chapter(&t.id, first.clone()).unwrap();
            if state == "paused" {
                s.pause_task(&t.id).unwrap();
            }
            if state == "failed" {
                s.fail_task(&t.id, "second failed").unwrap();
            }
            let saved = s
                .save_task_results(&t.id, &ids[..1], &first.package)
                .unwrap();
            assert_eq!(saved.status, state);
            assert_eq!(saved.saved_chapter_ids, ids[..1]);
            if state == "failed" {
                assert_eq!(saved.error.as_deref(), Some("second failed"));
            }
            if state != "running" {
                s.resume_task(&t.id).unwrap();
                s.mark_task_running(&t.id).unwrap();
            }
            let second = chapter_result(&d, 1, "second");
            assert_eq!(
                s.complete_chapter(&t.id, second.clone()).unwrap().status,
                "completed"
            );
            assert_eq!(
                s.save_task_results(&t.id, &ids[1..], &second.package)
                    .unwrap()
                    .status,
                "saved"
            );
        }
    }
    #[test]
    fn reanalysis_tracks_this_tasks_saves_and_repairs_legacy_status() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut s);
        let ids = d.chapters.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        for label in ["old", "new"] {
            let t = s.create_task(&d.id, d.version, ids.clone()).unwrap();
            s.mark_task_running(&t.id).unwrap();
            let first = chapter_result(&d, 0, label);
            let second = chapter_result(&d, 1, label);
            s.complete_chapter(&t.id, first.clone()).unwrap();
            s.complete_chapter(&t.id, second.clone()).unwrap();
            let partial = s
                .save_task_results(&t.id, &ids[..1], &first.package)
                .unwrap();
            assert_eq!(partial.status, "completed");
            if label == "new" {
                let mut legacy = json!(partial);
                legacy.as_object_mut().unwrap().remove("savedChapterIds");
                legacy["status"] = json!("saved");
                s.db.execute(
                    "UPDATE analysis_tasks SET body=? WHERE id=?",
                    params![legacy.to_string(), t.id],
                )
                .unwrap();
                s.recover_interrupted_tasks().unwrap();
                let repaired = s.task(&t.id).unwrap();
                assert_eq!(repaired.status, "completed");
                assert_eq!(repaired.saved_chapter_ids, ids[..1]);
            }
            assert_eq!(
                s.save_task_results(&t.id, &ids[1..], &second.package)
                    .unwrap()
                    .status,
                "saved"
            );
        }
        let t = s.create_task(&d.id, d.version, ids.clone()).unwrap();
        s.mark_task_running(&t.id).unwrap();
        s.complete_chapter(&t.id, chapter_result(&d, 0, "interrupted"))
            .unwrap();
        let mut legacy = json!(s.task(&t.id).unwrap());
        legacy.as_object_mut().unwrap().remove("savedChapterIds");
        legacy["status"] = json!("completed");
        s.db.execute(
            "UPDATE analysis_tasks SET body=? WHERE id=?",
            params![legacy.to_string(), t.id],
        )
        .unwrap();
        s.recover_interrupted_tasks().unwrap();
        assert_eq!(s.task(&t.id).unwrap().status, "paused");
        assert!(s.resume_task(&t.id).is_ok());
    }
    #[test]
    fn webdav_ignores_chapter_analysis_when_comparing_content() {
        let mut remote = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let d = fixture(&mut remote);
        let backup = remote.dispatch("snapshot", json!({})).unwrap();
        let mut local = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        local.merge_snapshot(backup.clone()).unwrap();
        let mut changed = backup;
        changed["documents"][0]["chapters"][0]["analysis"] = json!({
            "mainIdea":"不同理解", "overview":"不同概览", "keyPoints":[],
            "model":"other-model", "updatedAt":999
        });
        assert_eq!(local.merge_snapshot(changed).unwrap()["conflicts"], 0);
        assert!(local.document(&d.id).unwrap().chapters[0]
            .analysis
            .is_none());
    }
    #[test]
    fn source_key_trigger_rejects_raw_duplicates_and_allows_plain_documents() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let sourced: Document = decode(
            s.dispatch(
                "import_document",
                json!({"title":"原書","kind":"小说","source":{"key":"aozora:trigger"},
                  "chapters":[{"title":"一","text":"本文。"}]}),
            )
            .unwrap(),
        )
        .unwrap();
        let mut duplicate = sourced.clone();
        duplicate.id = id();
        let error =
            s.db.execute(
                "INSERT INTO documents(id,body) VALUES(?,?)",
                params![duplicate.id, serde_json::to_string(&duplicate).unwrap()],
            )
            .unwrap_err();
        assert!(error.to_string().contains("duplicate source key"));
        let plain = fixture(&mut s);
        assert!(plain.source.is_none());
        assert_eq!(s.all::<Document>("documents").unwrap().len(), 2);
    }
    #[test]
    fn source_validation() {
        let mut s = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        for url in ["javascript:alert(1)", "file:///C:/foo", "bad url"] {
            assert!(s.dispatch("import_document",json!({"title":"x","kind":"文章","url":url,"chapters":[{"title":"正文","text":"x"}]})).is_err());
        }
    }
}
#[cfg(test)]
mod reading_tests {
    use super::*;
    #[test]
    fn generated_readings_roundtrip_validation_and_backup() {
        let path = std::env::temp_dir().join(format!("japread-reading-{}.sqlite3", id()));
        let mut store = Store::open(&path).unwrap();
        let doc: Document = decode(store.dispatch("import_document", json!({"title":"注音测试","kind":"文章","chapters":[{"title":"一","text":"猫😀と今日。","readings":[{"start":0,"end":1,"reading":"ねこ"}]}]})).unwrap()).unwrap();
        let ch = &doc.chapters[0];
        let item = json!({"sentenceId":ch.sentences[0].id,"start":3,"end":5,"quote":"今日","reading":"きょう"});
        let payload =
            json!({"documentId":doc.id,"chapterId":ch.id,"version":1,"items":[item.clone()]});
        store.dispatch("readings_save", payload.clone()).unwrap();
        let mut bad = payload.clone();
        bad["items"][0]["start"] = json!(2);
        assert!(store.dispatch("readings_save", bad).is_err());
        let mut bad = payload.clone();
        bad["version"] = json!(2);
        assert!(store.dispatch("readings_save", bad).is_err());
        let mut bad = payload.clone();
        bad["items"] = json!([item.clone(), item]);
        assert!(store.dispatch("readings_save", bad).is_err());
        let mut bad = payload.clone();
        bad["items"][0] = json!({"sentenceId":ch.sentences[0].id,"start":0,"end":1,"quote":"猫","reading":"ねこ"});
        assert!(store.dispatch("readings_save", bad).is_err());
        drop(store);
        let mut store = Store::open(&path).unwrap();
        let saved = store.document(&doc.id).unwrap();
        assert_eq!(saved.chapters[0].generated_readings[0].reading, "きょう");
        assert_eq!(saved.chapters[0].sentences[0].readings[0].reading, "ねこ");
        assert!(store.all::<Annotation>("annotations").unwrap().is_empty());
        let snapshot = store.dispatch("snapshot", json!({})).unwrap();
        let mut other = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        other.merge_snapshot(snapshot).unwrap();
        assert_eq!(
            other.document(&doc.id).unwrap().chapters[0]
                .generated_readings
                .len(),
            1
        );
        let mut empty = payload;
        empty["items"] = json!([]);
        store.dispatch("readings_save", empty).unwrap();
        assert!(store.document(&doc.id).unwrap().chapters[0]
            .generated_readings
            .is_empty());
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}
#[cfg(test)]
mod paragraph_reading_tests {
    use super::*;
    #[test]
    fn newline_anchors_and_scoped_reading_save_preserve_other_paragraphs() {
        let mut store = Store::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let doc:Document = decode(store.dispatch("import_document",json!({"title":"段落","kind":"文章","chapters":[{"title":"正文","text":"猫。眠る。\r\n\r\n犬。\n鳥。"}]})).unwrap()).unwrap();
        let ch = &doc.chapters[0];
        assert_eq!(
            ch.sentences.iter().map(|s| s.paragraph).collect::<Vec<_>>(),
            vec![0, 0, 1, 2]
        );
        let a = json!({"sentenceId":ch.sentences[0].id,"start":0,"end":1,"quote":"猫","reading":"ねこ"});
        let b = json!({"sentenceId":ch.sentences[2].id,"start":0,"end":1,"quote":"犬","reading":"いぬ"});
        let request = |scope: Vec<usize>, items: Value| json!({"documentId":doc.id,"chapterId":ch.id,"version":1,"paragraphIds":scope,"items":items});
        store
            .dispatch("readings_save", request(vec![0], json!([a.clone()])))
            .unwrap();
        store
            .dispatch("readings_save", request(vec![1], json!([b])))
            .unwrap();
        assert_eq!(
            store.document(&doc.id).unwrap().chapters[0]
                .generated_readings
                .len(),
            2
        );
        assert!(store
            .dispatch("readings_save", request(vec![1], json!([a])))
            .is_err());
        store
            .dispatch("readings_save", request(vec![1], json!([])))
            .unwrap();
        assert_eq!(
            store.document(&doc.id).unwrap().chapters[0].generated_readings[0].quote,
            "猫"
        );
    }
}
