use crate::{
    llm::{client, limited_json, network_error},
    settings::{endpoint, DavConfig},
    Result,
};
use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Method, Url,
};
use serde_json::{json, Value};
use std::io::Read;
fn root(c: &DavConfig) -> Result<Url> {
    let mut u = endpoint(&c.endpoint)?;
    {
        let mut p = u.path_segments_mut().map_err(|_| "WebDAV 地址无效")?;
        p.pop_if_empty();
        for s in c.remote_path.split('/').filter(|s| !s.is_empty()) {
            if s == "." || s == ".." || s.contains(['\\', '?', '#', '%']) {
                return Err("远程目录无效".into());
            }
            p.push(s);
        }
        p.push("");
    }
    Ok(u)
}
fn auth(c: &DavConfig, b: RequestBuilder) -> RequestBuilder {
    if c.username.is_empty() && c.password.is_empty() {
        b
    } else {
        b.basic_auth(&c.username, Some(&c.password))
    }
}
fn request(c: &DavConfig, client: &Client, method: &str, url: Url) -> Result<RequestBuilder> {
    Ok(auth(
        c,
        client.request(
            Method::from_bytes(method.as_bytes()).map_err(|_| "无效请求方法")?,
            url,
        ),
    ))
}
fn check(r: Response) -> Result<Response> {
    if r.status().is_success() {
        Ok(r)
    } else {
        Err(match r.status().as_u16() {
            401 | 403 => "WebDAV 认证或权限失败".into(),
            404 => "远程目录或备份不存在；上传备份时会创建目录".into(),
            412 => "远程文件已存在或已变更，未覆盖".into(),
            _ => format!("WebDAV 返回 HTTP {}", r.status().as_u16()),
        })
    }
}
fn propfind(c: &DavConfig, depth: &str) -> Result<Response> {
    let client = client(60)?;
    let response=request(c,&client,"PROPFIND",root(c)?)?.header("Depth",depth).header("Content-Type","application/xml; charset=utf-8")
        .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:getcontentlength/><d:getlastmodified/><d:getetag/><d:resourcetype/></d:prop></d:propfind>"#)
        .send().map_err(network_error)?;
    check(response)
}
pub fn test(c: &DavConfig) -> Result<Value> {
    // Probe the configured server collection even if JapRead's child directory is new.
    let mut base = c.clone();
    base.remote_path = String::new();
    let r = propfind(&base, "0")?;
    if r.status().as_u16() != 207 {
        return Err("服务未返回 WebDAV 多状态响应，请检查是否填写了 WebDAV 地址".into());
    }
    Ok(json!({"message":"WebDAV 已连接；此检查只读取目录，上传权限会在备份时验证。"}))
}
fn create_directory(c: &DavConfig, client: &Client) -> Result<()> {
    let mut u = endpoint(&c.endpoint)?;
    for s in c.remote_path.split('/').filter(|s| !s.is_empty()) {
        {
            let mut p = u.path_segments_mut().map_err(|_| "目录地址无效")?;
            p.pop_if_empty();
            p.push(s);
            p.push("");
        }
        let response = request(c, client, "MKCOL", u.clone())?
            .send()
            .map_err(network_error)?;
        if response.status().as_u16() != 405 {
            check(response)?;
        }
    }
    Ok(())
}
pub fn upload(c: &DavConfig, snapshot: Value) -> Result<Value> {
    let client = client(120)?;
    let dir = root(c)?;
    create_directory(c, &client)?;
    let time = crate::now();
    let filename = format!("japread-{time}-{}.json", uuid::Uuid::new_v4());
    let bytes = serde_json::to_vec(
        &json!({"app":"JapRead","schemaVersion":1,"createdAt":time,"snapshot":snapshot}),
    )
    .map_err(|_| "备份编码失败")?;
    if bytes.len() > 50_000_000 {
        return Err("备份超过当前 50 MB 限制".into());
    }
    check(
        request(
            c,
            &client,
            "PUT",
            dir.join(&filename).map_err(|_| "备份路径无效")?,
        )?
        .header("If-None-Match", "*")
        .header("Content-Type", "application/json")
        .body(bytes.clone())
        .send()
        .map_err(network_error)?,
    )?;
    Ok(json!({"filename":filename,"bytes":bytes.len()}))
}
fn valid_name(name: &str) -> bool {
    name.starts_with("japread-")
        && name.ends_with(".json")
        && name.len() < 100
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}
pub fn list(c: &DavConfig) -> Result<Value> {
    let response = match propfind(c, "1") {
        Ok(r) => r,
        Err(e) if e.starts_with("远程目录") => return Ok(json!([])),
        Err(e) => return Err(e),
    };
    let mut xml = String::new();
    if response.status().as_u16() != 207 {
        return Err("服务没有返回 WebDAV 目录响应".into());
    }
    response
        .take(2_000_001)
        .read_to_string(&mut xml)
        .map_err(|_| "WebDAV 目录响应无法读取")?;
    if xml.len() > 2_000_000 {
        return Err("远程目录过大".into());
    }
    let base = root(c)?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut in_href = false;
    let mut href = String::new();
    let mut files = Vec::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) if e.local_name().as_ref() == b"href" => {
                in_href = true;
                href.clear();
            }
            Ok(quick_xml::events::Event::Text(e)) if in_href => {
                let text = e.decode().map_err(|_| "远程目录编码无效")?;
                href.push_str(&quick_xml::escape::unescape(&text).map_err(|_| "远程目录转义无效")?);
            }
            Ok(quick_xml::events::Event::End(e)) if e.local_name().as_ref() == b"href" => {
                in_href = false;
                if let Ok(u) = base.join(&href) {
                    if u.origin() == base.origin() {
                        if let Some(name) = u.path().strip_prefix(base.path()) {
                            if valid_name(name) {
                                files.push(name.to_owned());
                            }
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => return Err("WebDAV 目录 XML 无效".into()),
            _ => {}
        }
    }
    files.sort_by(|a, b| b.cmp(a));
    files.dedup();
    files.truncate(100);
    Ok(json!(files))
}
pub fn download(c: &DavConfig, filename: &str) -> Result<Value> {
    if !valid_name(filename) {
        return Err("备份文件名无效".into());
    }
    let response = request(
        c,
        &client(120)?,
        "GET",
        root(c)?.join(filename).map_err(|_| "备份路径无效")?,
    )?
    .send()
    .map_err(network_error)?;
    let value = limited_json(check(response)?, 50_000_000)?;
    if value["app"] != "JapRead" || value["schemaVersion"] != 1 {
        return Err("此文件不是支持的 JapRead 备份".into());
    }
    Ok(value["snapshot"].clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_paths_stay_below_configured_root() {
        let c = DavConfig {
            endpoint: "https://example.test/dav".into(),
            remote_path: "Books/JapRead".into(),
            ..Default::default()
        };
        assert_eq!(
            root(&c).unwrap().as_str(),
            "https://example.test/dav/Books/JapRead/"
        );
        assert!(!valid_name("../japread-x.json"));
        assert!(!valid_name("https://example.test/japread-x.json"));
        assert!(valid_name("japread-123-abc.json"));
    }
}
