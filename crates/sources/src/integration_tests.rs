//! Real loopback HTTP and an isolated Windows vault namespace; no paid provider calls.
use super::*;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};

struct Mock {
    address: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Mock {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut files = BTreeMap::<String, Vec<u8>>::new();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                if signal.load(Ordering::SeqCst) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut byte = [0];
                while !data.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    data.push(byte[0]);
                    assert!(data.len() < 20000);
                }
                let headers = String::from_utf8(data).unwrap();
                let lower = headers.to_lowercase();
                let mut parts = headers.lines().next().unwrap().split_whitespace();
                let method = parts.next().unwrap();
                let path = parts.next().unwrap();
                let length = lower
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                let (status, body) = match method {
                    "POST" => {
                        assert!(lower.contains("authorization: bearer test-only-key"));
                        let request: Value = serde_json::from_slice(&body).unwrap();
                        if request["model"] == "unauthorized" {
                            (401, b"sensitive-provider-error".to_vec())
                        } else {
                            let responses = path.ends_with("/responses");
                            let limit = if responses {
                                &request["max_output_tokens"]
                            } else {
                                &request["max_tokens"]
                            };
                            assert_eq!(
                                limit.as_u64(),
                                Some(if request["model"] == "batch-budget" {
                                    3072
                                } else {
                                    2048
                                })
                            );
                            let source: Value = serde_json::from_str(if responses {
                                request["input"][0]["content"].as_str().unwrap()
                            } else {
                                request["messages"][1]["content"].as_str().unwrap()
                            })
                            .unwrap();
                            let item = &source["sentences"][0];
                            let quote = if request["model"] == "bad-quote" {
                                "不存在"
                            } else {
                                "猫"
                            };
                            let focused = &source["selection"];
                            let occurrence = focused["occurrence"].as_u64().unwrap_or(1);
                            let text=json!({"items":[{"sentenceId":item["id"],"quote":quote,"occurrence":occurrence,"kind":"词汇","reading":"ねこ","meaning":"猫","explanation":"测试语境"}]}).to_string();
                            let result = if responses {
                                json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":text}]}]})
                            } else {
                                json!({"choices":[{"finish_reason":"stop","message":{"content":text}}]})
                            };
                            (200, serde_json::to_vec(&result).unwrap())
                        }
                    }
                    "PROPFIND" => {
                        assert!(lower.contains("authorization: basic "));
                        let hrefs = files
                            .keys()
                            .map(|p| format!("<d:response><d:href>{p}</d:href></d:response>"))
                            .collect::<String>();
                        (207,format!("<d:multistatus xmlns:d=\"DAV:\">{hrefs}<d:response><d:href>https://elsewhere.invalid/japread-evil.json</d:href></d:response></d:multistatus>").into_bytes())
                    }
                    "MKCOL" => (201, vec![]),
                    "PUT" => {
                        assert!(lower.contains("if-none-match: *"));
                        assert!(!files.contains_key(path));
                        let backup: Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(backup["app"], "JapRead");
                        assert!(backup.get("settings").is_none());
                        assert!(!String::from_utf8_lossy(&body).contains("test-only-key"));
                        files.insert(path.to_owned(), body);
                        (201, vec![])
                    }
                    "GET" => (200, files.get(path).unwrap().clone()),
                    _ => panic!("unexpected method"),
                };
                write!(
                    stream,
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        Self {
            address,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
struct Fixture {
    app: Option<Application>,
    path: PathBuf,
    namespace: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let s = settings::Settings::new(&self.namespace);
        let _ = s.clear("llm");
        let _ = s.clear("webdav");
        self.app.take();
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
#[test]
fn windows_vault_llm_preview_and_webdav_roundtrip() {
    let mock = Mock::start();
    let namespace = format!("test-{}", uuid::Uuid::new_v4());
    let path = std::env::temp_dir().join(&namespace);
    std::fs::create_dir_all(&path).unwrap();
    let fixture = Fixture {
        app: Some(
            Application::open(&path.join("db.sqlite3"), &path.join("cache"), &namespace).unwrap(),
        ),
        path,
        namespace,
    };
    let app = fixture.app.as_ref().unwrap();
    let mut config = json!({"endpoint":format!("http://{}",mock.address),"model":"test","protocol":"chat","apiKey":"test-only-key","maxTokens":2048});
    let public = app
        .dispatch("settings_save", json!({"section":"llm","config":config}))
        .unwrap();
    assert_eq!(public["hasApiKey"], true);
    assert_eq!(public["llm"]["apiKey"], "");
    assert_eq!(
        settings::Settings::new(&fixture.namespace)
            .llm()
            .unwrap()
            .api_key,
        "test-only-key"
    );
    config["apiKey"] = json!("");
    app.dispatch("settings_save", json!({"section":"llm","config":config}))
        .unwrap();
    app.dispatch("llm_test", json!({})).unwrap();
    let doc=app.dispatch("import_document",json!({"title":"测试作品","kind":"文章","chapters":[{"title":"正文","text":"猫が眠っています。"}]})).unwrap();
    let input = json!({"documentId":doc["id"],"chapterId":doc["chapters"][0]["id"],"sentenceIds":[doc["chapters"][0]["sentences"][0]["id"]]});
    let result = app.dispatch("llm_analyze", input.clone()).unwrap();
    assert_eq!(
        app.dispatch("snapshot", json!({})).unwrap()["annotations"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        app.dispatch("import_annotations", result["package"].clone())
            .unwrap()["added"],
        1
    );
    assert_eq!(
        app.dispatch("import_annotations", result["package"].clone())
            .unwrap()["added"],
        0
    );
    config["protocol"] = json!("responses");
    app.dispatch("settings_save", json!({"section":"llm","config":config}))
        .unwrap();
    app.dispatch("llm_analyze", input.clone()).unwrap();
    // Each independent batch retains the configured budget in both protocols.
    for protocol in ["chat", "responses"] {
        config["protocol"] = json!(protocol);
        config["model"] = json!("batch-budget");
        config["maxTokens"] = json!(3072);
        app.dispatch("settings_save", json!({"section":"llm","config":config}))
            .unwrap();
        for _ in 0..2 {
            app.dispatch("llm_analyze", input.clone()).unwrap();
        }
        let mut focused = input.clone();
        focused["target"] = json!({"sentenceId":input["sentenceIds"][0],"start":0,"end":1});
        let preview = app.dispatch("llm_selection", focused.clone()).unwrap();
        assert_eq!(preview["package"]["annotations"][0]["quote"], "猫");
        focused["target"]["end"] = json!(2);
        assert!(app.dispatch("llm_selection", focused.clone()).is_err());
        focused["target"]["end"] = json!(999);
        assert!(app.dispatch("llm_selection", focused).is_err());
    }
    config["maxTokens"] = json!(2048);
    for model in ["bad-quote", "unauthorized"] {
        config["model"] = json!(model);
        app.dispatch("settings_save", json!({"section":"llm","config":config}))
            .unwrap();
        let e = app.dispatch("llm_analyze", input.clone()).unwrap_err();
        assert!(!e.contains("sensitive-provider-error"));
        assert!(!e.contains(&mock.address));
    }
    app.dispatch("settings_save",json!({"section":"webdav","config":{"endpoint":format!("http://{}/dav",mock.address),"username":"test","password":"test-only-password","remotePath":"JapRead"}})).unwrap();
    app.dispatch("webdav_test", json!({})).unwrap();
    let first = app.dispatch("webdav_upload", json!({})).unwrap();
    let second = app.dispatch("webdav_upload", json!({})).unwrap();
    assert_ne!(first["filename"], second["filename"]);
    assert_eq!(
        app.dispatch("webdav_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let result = app
        .dispatch("webdav_restore", json!({"filename":first["filename"]}))
        .unwrap();
    assert_eq!(result["documents"], 0);
    assert_eq!(result["annotations"], 0);
    assert!(app
        .dispatch("webdav_restore", json!({"filename":"../outside.json"}))
        .is_err());
    app.dispatch("settings_clear", json!({"section":"llm"}))
        .unwrap();
    assert_eq!(
        app.dispatch("settings_get", json!({})).unwrap()["hasApiKey"],
        false
    );
}
