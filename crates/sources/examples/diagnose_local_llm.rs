//! Opt-in model diagnostic. Remote calls additionally require --allow-remote.
//! Never writes credentials, responses, or library data; overrides are in-memory only.
#![allow(dead_code)]
#[path = "../src/llm.rs"]
mod llm;
#[path = "../src/settings.rs"]
mod settings;
type Result<T> = std::result::Result<T, String>;
use japread_core::{Document, Sentence};
use serde_json::json;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let filter = args
        .windows(2)
        .find(|w| w[0] == "--model")
        .map(|w| w[1].as_str());
    let mut c = if args.iter().any(|a| a == "--list") || filter.is_some() {
        let entry = keyring::Entry::new("com.japread.desktop", "llm-profiles")
            .map_err(|_| "无法读取索引")?;
        let index: serde_json::Value =
            serde_json::from_slice(&entry.get_secret().map_err(|_| "无法读取索引")?)
                .map_err(|_| "索引格式无效")?;
        let mut selected = None;
        for id in index["ids"].as_array().ok_or("缺少配置列表")? {
            let entry = keyring::Entry::new(
                "com.japread.desktop",
                &format!("llm-profile-{}", id.as_str().ok_or("配置 ID 无效")?),
            )
            .map_err(|_| "无法读取配置")?;
            let profile: serde_json::Value =
                serde_json::from_slice(&entry.get_secret().map_err(|_| "无法读取配置")?)
                    .map_err(|_| "配置格式无效")?;
            let c: settings::LlmConfig =
                serde_json::from_value(profile["config"].clone()).map_err(|_| "配置格式无效")?;
            let url = settings::endpoint(&c.endpoint)?;
            if args.iter().any(|a| a == "--list") {
                println!(
                    "{}",
                    json!({"model":c.model,"protocol":c.protocol,"local":matches!(url.host_str(),Some("localhost"|"127.0.0.1"|"[::1]")),"maxTokens":c.max_tokens,"timeoutSeconds":c.timeout_seconds,"structured":c.structured})
                );
            }
            if filter.is_some_and(|f| c.model.to_lowercase().contains(&f.to_lowercase())) {
                if selected.is_some() {
                    return Err("匹配多套配置，请使用更精确的模型名".into());
                }
                selected = Some(c);
            }
        }
        if args.iter().any(|a| a == "--list") {
            return Ok(());
        }
        selected.ok_or("没有匹配的模型配置")?
    } else {
        settings::Settings::new("desktop").llm()?
    };
    if let Some(limit) = args.windows(2).find(|w| w[0] == "--limit") {
        c.max_tokens = limit[1]
            .parse::<u32>()
            .map_err(|_| "输出上限无效")?
            .min(c.max_tokens);
    }
    if args.iter().any(|a| a == "--structured") {
        c.structured = true;
    }
    let url = settings::endpoint(&c.endpoint)?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !local && !args.iter().any(|a| a == "--allow-remote") {
        return Err("当前配置不是本机服务；诊断工具不会请求远程模型或更改当前配置".into());
    }
    println!(
        "{}",
        json!({"model":c.model,"local":local,"protocol":c.protocol,"maxTokens":c.max_tokens,"timeoutSeconds":c.timeout_seconds,"structured":c.structured,"temperature":serde_json::from_str::<serde_json::Value>(&c.extra_body).ok().and_then(|v|v["temperature"].as_f64())})
    );
    if !std::env::args().any(|arg| arg == "--run") {
        return Ok(());
    }
    let mut doc: Document = serde_json::from_value(json!({"id":"diagnostic","title":"诊断短文","author":"","kind":"文章","url":"","version":1,"chapters":[]})).map_err(|_| "诊断数据无效")?;
    let mut sentences = vec![
        Sentence {
            id: "4fc8b7b9-e157-4627-a116-b565cbcae226".into(),
            text: "静かな雨が降っています。".into(),
            paragraph: 0,
            readings: vec![],
        },
        Sentence {
            id: "e2106d90-0c94-4dd6-8d25-84b9f3f5a9a1".into(),
            text: "知らない言葉に出会うたびに、少しだけ立ち止まる。".into(),
            paragraph: 0,
            readings: vec![],
        },
    ];
    if args.iter().any(|a| a == "--second-batch") {
        sentences[0].id = "566c0d61-467b-4447-a1bb-02ca1da9c2de".into();
        sentences[0].text = "窓を開けると、冷たい風が部屋に入ってきた。".into();
        sentences[1].id = "ae1cf158-d399-4d60-91f0-4f2d5c58f55b".into();
        sentences[1].text = "本を読み終えたあとで、知らない言葉をノートに書き留めた。".into();
    }
    if let Some(work) = args.windows(2).find(|w| w[0] == "--work") {
        let app = japread_sources::Application::open(
            std::path::Path::new(".local/llm-regression.sqlite3"),
            std::path::Path::new(".local/sources"),
            "diagnostic-library",
        )?;
        let imported = app.dispatch(
            "source_import",
            json!({"provider":"aozora","workId":work[1]}),
        )?;
        doc =
            serde_json::from_value(imported["document"].clone()).map_err(|_| "诊断作品格式无效")?;
        println!(
            "{}",
            json!({"work":doc.title,"chapters":doc.chapters.iter().map(|ch|json!({"title":ch.title,"sentences":ch.sentences.len()})).collect::<Vec<_>>()})
        );
        let chapter = args
            .windows(2)
            .find(|w| w[0] == "--chapter")
            .map(|w| w[1].parse::<usize>())
            .transpose()
            .map_err(|_| "章节索引无效")?
            .unwrap_or(1);
        let offset = args
            .windows(2)
            .find(|w| w[0] == "--offset")
            .map(|w| w[1].parse::<usize>())
            .transpose()
            .map_err(|_| "句子偏移无效")?
            .unwrap_or(0);
        let size = args
            .windows(2)
            .find(|w| w[0] == "--size")
            .map(|w| w[1].parse::<usize>())
            .transpose()
            .map_err(|_| "批次大小无效")?
            .unwrap_or(2);
        if !(1..=8).contains(&size) {
            return Err("每批需 1–8 句".into());
        }
        sentences = doc
            .chapters
            .get(chapter.saturating_sub(1))
            .ok_or("没有该章节")?
            .sentences
            .iter()
            .skip(offset)
            .take(size)
            .cloned()
            .collect();
        if sentences.is_empty() {
            return Err("所选诊断批次没有句子".into());
        }
        println!(
            "{}",
            json!({"chapter":chapter,"offset":offset,"batchSentences":sentences.len()})
        );
    }
    let body = llm::payload(&c, &sentences)?;
    let mut request = llm::client(c.timeout_seconds)?
        .post(llm::request_url(&c)?)
        .json(&body);
    if !c.api_key.is_empty() {
        request = request.bearer_auth(&c.api_key);
    }
    let started = std::time::Instant::now();
    let response = llm::limited_json(request.send().map_err(llm::network_error)?, 2_000_000)?;
    println!(
        "{}",
        json!({"elapsedMs":started.elapsed().as_millis(),"status":response["status"],"incompleteReason":response.pointer("/incomplete_details/reason"),"finishReason":response.pointer("/choices/0/finish_reason"),"usage":response.get("usage")})
    );
    let text = llm::response_text(&response, &c.protocol)?;
    let raw = text
        .trim()
        .strip_prefix("```json")
        .or_else(|| text.trim().strip_prefix("```"))
        .unwrap_or(text.trim())
        .trim()
        .trim_end_matches("```")
        .trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(items) = value["items"].as_array() {
            for (i, item) in items.iter().enumerate() {
                println!(
                    "{}",
                    json!({"item":i+1,"sentenceId":item["sentenceId"].as_str().map(|s|s.chars().take(50).collect::<String>()),"kind":item["kind"].as_str().map(|s|s.chars().take(30).collect::<String>()),"quoteEmpty":item["quote"].as_str().is_none_or(|s|s.trim().is_empty()),"meaningEmpty":item["meaning"].as_str().is_none_or(|s|s.trim().is_empty()),"occurrence":item["occurrence"].as_u64()})
                );
            }
        }
    }
    let (package, warnings) = llm::review_response(&text, &doc, &sentences)?;
    println!(
        "{}",
        json!({"validatedAnnotations":package.annotations.len(),"warnings":warnings,"saved":false})
    );
    Ok(())
}
