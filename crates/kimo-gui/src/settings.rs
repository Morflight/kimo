//! Non-secret settings, kept as JSON in the app's config dir. API keys are NEVER stored here (see keys.rs).
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// MCP switch: off by default. On = the engine was verified and the external-activity listener runs.
    pub mcp_enabled: bool,
    pub provider: String,
    /// Per-provider model and base URL overrides; empty = the provider's default (REFERENCE ONLY defaults).
    pub models: HashMap<String, String>,
    pub base_urls: HashMap<String, String>,
    /// Providers whose key the user asked to remember (the key itself is in the OS credential store).
    pub remembered: Vec<String>,
    /// The first-run setup was finished or skipped.
    pub setup_done: bool,
    /// Which agent the Agent tab talks to: "claude-code" | "codex" | "antigravity" | "api" (own key).
    pub agent: String,
    /// Optional model per subscription agent ("" = the CLI's own default).
    pub agent_models: HashMap<String, String>,
    /// Optional reasoning effort per subscription agent ("" = the CLI's own default).
    pub agent_efforts: HashMap<String, String>,
    /// Recently opened or saved project files, newest first.
    pub recent: Vec<String>,
    /// The welcome tour was finished or skipped.
    pub onboarded: bool,
    /// Release builds ask GitHub releases for a newer kimo shortly after start (installing always waits for a click).
    pub update_check: bool,
}
impl Default for Settings {
    fn default() -> Self { Settings { mcp_enabled: false, provider: "openai".into(), models: HashMap::new(), base_urls: HashMap::new(), remembered: vec![], setup_done: false,
        agent: "claude-code".into(), agent_models: HashMap::new(), agent_efforts: HashMap::new(), recent: vec![], onboarded: false, update_check: true } }
}
impl Settings {
    pub fn load(p: &Path) -> Settings { std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() }
    /// Put a project path at the top of the recent list (max 8, no duplicates).
    pub fn touch_recent(&mut self, path: &str) {
        self.recent.retain(|r| r != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(8);
    }
    pub fn save(&self, p: &Path) -> Result<(), String> {
        self.save_encoded(p, &serde_json::to_vec_pretty)
    }
    // Keep serialization failure handling testable without corrupting files or
    // depending on allocation failure. Production always uses serde JSON above.
    fn save_encoded(&self, p: &Path, encode: &dyn Fn(&Settings) -> Result<Vec<u8>, serde_json::Error>) -> Result<(), String> {
        if let Some(d) = p.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, encode(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, p).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_off_and_round_trip() {
        let d = std::env::temp_dir().join(format!("kimo-settings-{}", std::process::id()));
        let p = d.join("settings.json");
        let s = Settings::load(&p);
        assert!(!s.mcp_enabled, "MCP is off by default");
        let mut s2 = s.clone(); s2.mcp_enabled = true; s2.remembered.push("openai".into());
        s2.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s2);
        assert!(!std::fs::read_to_string(&p).unwrap().contains("sk-"));
        std::fs::write(&p, "{\"provider\":\"groq\"}").unwrap();
        assert_eq!(Settings::load(&p).provider, "groq");
        assert_eq!(Settings::load(&p).agent, "claude-code", "old settings files get the new defaults");
        let mut r = Settings::default();
        for i in 0..10 { r.touch_recent(&format!("p{i}")); }
        r.touch_recent("p5");
        assert_eq!(r.recent.len(), 8);
        assert_eq!(r.recent[0], "p5");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn corrupt_or_wrongly_typed_settings_fall_back_without_overwriting_the_file() {
        let d = std::env::temp_dir().join(format!("kimo-settings-invalid-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("settings.json");
        for text in ["not json", "[]", "{\"mcp_enabled\":\"yes\"}"] {
            std::fs::write(&p, text).unwrap();
            assert_eq!(Settings::load(&p), Settings::default());
            assert_eq!(std::fs::read_to_string(&p).unwrap(), text);
        }
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn save_reports_parent_write_and_rename_errors_without_losing_existing_settings() {
        let d = std::env::temp_dir().join(format!("kimo-settings-errors-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let blocker = d.join("not-a-directory");
        std::fs::write(&blocker, "keep parent").unwrap();
        let settings = Settings::default();
        assert!(!settings.save(&blocker.join("settings.json")).unwrap_err().is_empty());
        assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "keep parent");

        let target = d.join("settings.json");
        std::fs::write(&target, "previous settings").unwrap();
        let temporary = target.with_extension("json.tmp");
        std::fs::create_dir(&temporary).unwrap();
        assert!(!settings.save(&target).unwrap_err().is_empty());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "previous settings");
        std::fs::remove_dir(&temporary).unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep"), "preserved").unwrap();
        assert!(!settings.save(&target).unwrap_err().is_empty());
        assert_eq!(std::fs::read_to_string(target.join("keep")).unwrap(), "preserved");
        assert_eq!(Settings::load(&temporary), settings);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn all_preferences_survive_unicode_round_trip_and_recent_paths_remain_unique() {
        let d = std::env::temp_dir().join(format!("kimo-settings-unicode-{}", std::process::id()));
        let p = d.join("nested/settings.json");
        let mut settings = Settings::default();
        settings.mcp_enabled = true;
        settings.provider = "anthropic".into();
        settings.models.insert("anthropic".into(), "modèle".into());
        settings.base_urls.insert("anthropic".into(), "http://127.0.0.1:1234".into());
        settings.remembered.push("anthropic".into());
        settings.setup_done = true;
        settings.agent = "codex".into();
        settings.agent_models.insert("codex".into(), "modèle".into());
        settings.agent_efforts.insert("codex".into(), "high".into());
        settings.onboarded = true;
        settings.update_check = false;
        settings.touch_recent("/projets/été 🎬.json");
        settings.touch_recent("/projets/second.json");
        settings.touch_recent("/projets/été 🎬.json");
        assert_eq!(settings.recent, ["/projets/été 🎬.json", "/projets/second.json"]);
        settings.save(&p).unwrap();
        assert_eq!(Settings::load(&p), settings);
        assert!(!p.with_extension("json.tmp").exists());
        let serialized: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert!(serialized.get("keys").is_none());
        assert_eq!(serialized["remembered"], serde_json::json!(["anthropic"]));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn serialization_failure_and_empty_destination_are_reported_without_replacement() {
        let d = std::env::temp_dir().join(format!("kimo-settings-encode-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("settings.json");
        std::fs::write(&p, "keep this file").unwrap();
        let settings = Settings::default();
        let fail = |_: &Settings| Err(<serde_json::Error as serde::ser::Error>::custom("synthetic encoding failure"));
        assert_eq!(settings.save_encoded(&p, &fail).unwrap_err(), "synthetic encoding failure");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "keep this file");
        assert!(!p.with_extension("json.tmp").exists());
        assert!(settings.save(Path::new("")).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }

}
