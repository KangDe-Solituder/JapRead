use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct LlmConfig {
    pub endpoint: String,
    pub model: String,
    pub protocol: String,
    pub api_key: String,
    pub timeout_seconds: u64,
    pub max_tokens: u32,
    pub structured: bool,
    pub extra_body: String,
}
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            model: String::new(),
            protocol: "chat".into(),
            api_key: String::new(),
            timeout_seconds: 90,
            max_tokens: 6000,
            structured: false,
            extra_body: String::new(),
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct DavConfig {
    pub endpoint: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct ProfileIndex {
    active: Option<String>,
    ids: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct LlmProfile {
    name: String,
    config: LlmConfig,
}

// Connection profiles, including addresses and usernames, live only in the OS vault.
// The Web development adapter uses a separate namespace from the desktop app.
pub struct Settings {
    service: String,
}
impl Settings {
    pub fn new(namespace: &str) -> Self {
        Self {
            service: format!("com.japread.{namespace}"),
        }
    }
    #[cfg(windows)]
    fn read(&self, name: &str) -> Result<Option<String>> {
        match keyring::Entry::new(&self.service, name)
            .map_err(|_| "无法访问 Windows 凭据管理器")?
            .get_secret()
        {
            Ok(v) => String::from_utf8(v)
                .map(Some)
                .map_err(|_| "凭据编码无效，请重新配置".into()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("无法读取 Windows 凭据，请检查当前 Windows 用户会话".into()),
        }
    }
    #[cfg(not(windows))]
    fn read(&self, _: &str) -> Result<Option<String>> {
        Ok(None)
    }
    #[cfg(windows)]
    fn write(&self, name: &str, value: &str) -> Result<()> {
        if value.len() > 2400 {
            return Err("连接配置过长，请缩短地址或高级参数（总计最多 2400 UTF-8 字节）".into());
        }
        keyring::Entry::new(&self.service, name)
            .map_err(|_| "无法访问 Windows 凭据管理器")?
            .set_secret(value.as_bytes())
            .map_err(|_| "Windows 凭据保存失败；未回退到明文文件".into())
    }
    #[cfg(not(windows))]
    fn write(&self, _: &str, _: &str) -> Result<()> {
        Err("此版本的安全配置保存仅支持 Windows".into())
    }
    pub fn clear(&self, name: &str) -> Result<()> {
        if !["llm", "webdav"].contains(&name) {
            return Err("未知配置分区".into());
        }
        if name == "llm" {
            let index = self.profile_index()?;
            for id in &index.ids {
                self.remove_profile(id)?;
            }
            self.erase("llm-profiles")?;
        }
        self.erase(name)
    }
    fn erase(&self, name: &str) -> Result<()> {
        #[cfg(windows)]
        {
            match keyring::Entry::new(&self.service, name)
                .map_err(|_| "无法访问 Windows 凭据管理器")?
                .delete_credential()
            {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err("无法移除 Windows 凭据".into()),
            }
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }
    pub fn llm(&self) -> Result<LlmConfig> {
        let index = self.profile_index()?;
        match index.active {
            Some(id) => Ok(self.profile(&id)?.config),
            None => Ok(LlmConfig::default()),
        }
    }
    fn legacy_llm(&self) -> Result<LlmConfig> {
        self.read("llm")?
            .map(|s| serde_json::from_str(&s).map_err(|_| "LLM 凭据格式损坏，请重新配置".into()))
            .unwrap_or_else(|| Ok(LlmConfig::default()))
    }
    fn profile_index(&self) -> Result<ProfileIndex> {
        if let Some(raw) = self.read("llm-profiles")? {
            return serde_json::from_str(&raw)
                .map_err(|_| "模型配置索引损坏，请检查 Windows 凭据".into());
        }
        let legacy = self.legacy_llm()?;
        if legacy.endpoint.is_empty() {
            return Ok(ProfileIndex::default());
        }
        // Fixed migration ID makes retries idempotent. Remove the old credential only
        // after both the new profile and its index have been persisted successfully.
        self.write(
            "llm-profile-legacy",
            &serde_json::to_string(&LlmProfile {
                name: "原有配置".into(),
                config: legacy,
            })
            .map_err(|_| "配置编码失败")?,
        )?;
        let index = ProfileIndex {
            active: Some("legacy".into()),
            ids: vec!["legacy".into()],
        };
        self.write_index(&index)?;
        self.erase("llm")?;
        Ok(index)
    }
    fn write_index(&self, index: &ProfileIndex) -> Result<()> {
        self.write(
            "llm-profiles",
            &serde_json::to_string(index).map_err(|_| "配置编码失败")?,
        )
    }
    fn profile(&self, id: &str) -> Result<LlmProfile> {
        self.read(&format!("llm-profile-{id}"))?
            .ok_or("模型配置不存在".into())
            .and_then(|s| serde_json::from_str(&s).map_err(|_| "模型配置格式损坏".into()))
    }
    pub fn activate_profile(&self, id: &str) -> Result<Value> {
        let mut index = self.profile_index()?;
        if !index.ids.iter().any(|x| x == id) {
            return Err("模型配置不存在".into());
        }
        self.profile(id)?;
        index.active = Some(id.into());
        self.write_index(&index)?;
        self.public()
    }
    pub fn remove_profile(&self, id: &str) -> Result<Value> {
        let mut index = self.profile_index()?;
        if !index.ids.iter().any(|x| x == id) {
            return Err("模型配置不存在".into());
        }
        index.ids.retain(|x| x != id);
        if index.active.as_deref() == Some(id) {
            index.active = None;
        }
        self.write_index(&index)?;
        self.erase(&format!("llm-profile-{id}"))?;
        self.public()
    }
    pub fn save_profile(
        &self,
        id: Option<&str>,
        name: &str,
        value: Value,
        clear_key: bool,
    ) -> Result<Value> {
        let mut index = self.profile_index()?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err("配置名称需 1–40 个字".into());
        }
        let mut config: LlmConfig =
            serde_json::from_value(value).map_err(|_| "LLM 配置格式不正确")?;
        let existing = if let Some(id) = id {
            if !index.ids.iter().any(|x| x == id) {
                return Err("模型配置不存在".into());
            }
            Some(self.profile(id)?.config)
        } else {
            None
        };
        if id.is_none() && index.ids.len() >= 20 {
            return Err("最多保存 20 套模型配置".into());
        }
        config.endpoint = config.endpoint.trim().trim_end_matches('/').into();
        config.model = config.model.trim().into();
        endpoint(&config.endpoint)?;
        if !["chat", "responses"].contains(&config.protocol.as_str())
            || config.model.is_empty()
            || config.model.len() > 150
            || !(15..=180).contains(&config.timeout_seconds)
            || !(512..=16000).contains(&config.max_tokens)
        {
            return Err("请填写模型，超时 15–180 秒，输出上限 512–16000 tokens".into());
        }
        validate_extra(&config.extra_body)?;
        config.api_key = config
            .api_key
            .trim()
            .strip_prefix("Bearer ")
            .unwrap_or(config.api_key.trim())
            .to_owned();
        if clear_key {
            config.api_key.clear();
        } else if config.api_key.is_empty() {
            if let Some(old) = existing {
                if old.endpoint != config.endpoint && !old.api_key.is_empty() {
                    return Err(
                        "接口地址已改变，请重新填写密钥或勾选清除密钥，避免将原密钥发送给另一服务"
                            .into(),
                    );
                }
                config.api_key = old.api_key;
            }
        }
        if config.api_key.chars().any(char::is_whitespace) {
            return Err("API Key 不能包含空白字符".into());
        }
        let profile_id = id
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.write(
            &format!("llm-profile-{profile_id}"),
            &serde_json::to_string(&LlmProfile {
                name: name.into(),
                config,
            })
            .map_err(|_| "配置编码失败")?,
        )?;
        if id.is_none() {
            index.ids.push(profile_id.clone());
            if index.active.is_none() {
                index.active = Some(profile_id.clone());
            }
            if let Err(e) = self.write_index(&index) {
                let _ = self.erase(&format!("llm-profile-{profile_id}"));
                return Err(e);
            }
        }
        self.public()
    }
    pub fn dav(&self) -> Result<DavConfig> {
        self.read("webdav")?
            .map(|s| serde_json::from_str(&s).map_err(|_| "WebDAV 凭据格式损坏，请重新配置".into()))
            .unwrap_or_else(|| Ok(DavConfig::default()))
    }
    pub fn public(&self) -> Result<Value> {
        let mut llm = self.llm()?;
        let mut dav = self.dav()?;
        let has_key = !llm.api_key.is_empty();
        let has_password = !dav.password.is_empty();
        llm.api_key.clear();
        dav.password.clear();
        let index = self.profile_index()?;
        let mut profiles = Vec::new();
        for id in &index.ids {
            let mut profile = self.profile(id)?;
            let has_key = !profile.config.api_key.is_empty();
            profile.config.api_key.clear();
            profiles.push(
                json!({"id":id,"name":profile.name,"config":profile.config,"hasApiKey":has_key}),
            );
        }
        Ok(
            json!({"llm":llm,"llmProfiles":profiles,"activeLlmId":index.active,"webdav":dav,"hasApiKey":has_key,"hasDavPassword":has_password,"secureStorage":cfg!(windows)}),
        )
    }
    pub fn save(&self, kind: &str, value: Value) -> Result<Value> {
        match kind {
            "llm" => {
                let index = self.profile_index()?;
                let name = match &index.active {
                    Some(id) => self.profile(id)?.name,
                    None => "默认配置".into(),
                };
                return self.save_profile(index.active.as_deref(), &name, value, false);
            }
            "webdav" => {
                let mut config: DavConfig =
                    serde_json::from_value(value).map_err(|_| "WebDAV 配置格式不正确")?;
                config.endpoint = config.endpoint.trim().trim_end_matches('/').into();
                config.username = config.username.trim().into();
                endpoint(&config.endpoint)?;
                config.remote_path = config.remote_path.trim().trim_matches('/').into();
                if config.remote_path.is_empty() {
                    config.remote_path = "JapRead".into();
                }
                if config.remote_path.split('/').any(|s| {
                    s.is_empty() || s == "." || s == ".." || s.contains(['\\', '?', '#', '%'])
                }) {
                    return Err("远程目录请使用普通目录名，以 / 分隔".into());
                }
                if config.password.is_empty() {
                    config.password = self.dav()?.password;
                }
                self.write(
                    "webdav",
                    &serde_json::to_string(&config).map_err(|_| "配置编码失败")?,
                )?;
            }
            _ => return Err("未知配置分区".into()),
        }
        self.public()
    }
}
pub fn endpoint(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| "请输入完整服务地址")?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err("服务地址需 HTTPS（本机 localhost 可用 HTTP），账号和密钥请填独立字段".into());
    }
    Ok(url)
}
pub fn validate_extra(raw: &str) -> Result<Value> {
    let value = if raw.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(raw).map_err(|_| "高级参数必须是 JSON 对象")?
    };
    let obj = value.as_object().ok_or("高级参数必须是 JSON 对象")?;
    // Never allow generic overrides to replace the source, output protocol, or authorization.
    const ALLOWED: &[&str] = &[
        "temperature",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
        "seed",
        "reasoning_effort",
        "reasoning",
        "thinking",
        "enable_thinking",
    ];
    if obj.keys().any(|k| !ALLOWED.contains(&k.as_str())) {
        return Err(
            "高级参数仅支持 temperature、top_p、penalty、seed、reasoning、thinking 等推理参数"
                .into(),
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn profiles_migrate_isolate_secrets_and_switch() {
        struct Cleanup(Settings);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.clear("llm");
            }
        }
        let guard = Cleanup(Settings::new(&format!(
            "test-profiles-{}",
            uuid::Uuid::new_v4()
        )));
        let s = &guard.0;
        let original = LlmConfig {
            endpoint: "https://first.example/v1".into(),
            model: "first".into(),
            api_key: "first-secret".into(),
            ..Default::default()
        };
        s.write("llm", &serde_json::to_string(&original).unwrap())
            .unwrap();
        let initial = s.public().unwrap();
        assert_eq!(initial["activeLlmId"], "legacy");
        assert_eq!(initial["llmProfiles"][0]["name"], "原有配置");
        assert!(!initial.to_string().contains("first-secret"));
        assert!(s.read("llm").unwrap().is_none());
        assert_eq!(s.llm().unwrap().api_key, "first-secret");
        // Reading again does not duplicate migrated profiles.
        assert_eq!(
            s.public().unwrap()["llmProfiles"].as_array().unwrap().len(),
            1
        );
        let second = json!({"endpoint":"http://127.0.0.1:8080/v1","model":"local"});
        let added = s
            .save_profile(None, "本机模型", second.clone(), false)
            .unwrap();
        let id = added["llmProfiles"][1]["id"].as_str().unwrap();
        assert_eq!(added["activeLlmId"], "legacy");
        s.activate_profile(id).unwrap();
        assert!(s.llm().unwrap().api_key.is_empty());
        s.activate_profile("legacy").unwrap();
        let mut edit = serde_json::to_value(original).unwrap();
        edit["apiKey"] = json!("");
        s.save_profile(Some("legacy"), "重命名", edit.clone(), false)
            .unwrap();
        assert_eq!(s.llm().unwrap().api_key, "first-secret");
        edit["endpoint"] = json!("https://different.example/v1");
        assert!(s
            .save_profile(Some("legacy"), "不同服务", edit.clone(), false)
            .is_err());
        assert_eq!(s.llm().unwrap().endpoint, "https://first.example/v1");
        s.save_profile(Some("legacy"), "免鉴权", edit, true)
            .unwrap();
        assert!(s.llm().unwrap().api_key.is_empty());
        s.remove_profile("legacy").unwrap();
        assert!(s.public().unwrap()["activeLlmId"].is_null());
        assert!(s.llm().unwrap().endpoint.is_empty());
        assert!(s.activate_profile("unknown").is_err());
        s.activate_profile(id).unwrap();
        assert_eq!(s.llm().unwrap().model, "local");
    }
    #[test]
    fn connection_urls_and_overrides_are_bounded() {
        assert!(endpoint("https://example.test/v1").is_ok());
        assert!(endpoint("http://127.0.0.1:9999/v1").is_ok());
        for u in [
            "http://example.test",
            "https://u:p@example.test",
            "https://example.test?key=secret",
        ] {
            assert!(endpoint(u).is_err());
        }
        assert!(validate_extra(r#"{"model":"other"}"#).is_err());
        assert!(validate_extra(r#"{"temperature":0.2}"#).is_ok());
    }
}
