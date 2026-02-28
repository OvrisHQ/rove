//! Steering Engine
//!
//! Loads and manages Agent Skills from TOML and Markdown files.
//! Skills shape agent behavior — how it thinks, what it prioritizes,
//! what tools it prefers — without changing code.
//!
//! Supports:
//! - TOML skill files with full activation/routing/directive config
//! - Legacy Markdown skill files with YAML frontmatter
//! - Manual and auto-activation based on task content
//! - Conflict resolution when multiple skills are active
//! - Merged directives for context injection

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// Maximum priority allowed for user-created skills
const MAX_USER_PRIORITY: u16 = 90;

/// An Agent Skill loaded from a TOML or Markdown file
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub file_path: PathBuf,
    /// Full TOML config (None for legacy .md skills)
    pub config: Option<SkillFile>,
}

/// TOML skill file schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    pub meta: SkillMeta,
    #[serde(default)]
    pub activation: SkillActivation,
    #[serde(default)]
    pub directives: SkillDirectives,
    #[serde(default)]
    pub routing: SkillRouting,
    #[serde(default)]
    pub tools: SkillTools,
    #[serde(default)]
    pub memory: SkillMemory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillActivation {
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub auto_when: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default = "default_priority")]
    pub priority: u16,
}

fn default_priority() -> u16 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillDirectives {
    #[serde(default)]
    pub system_prefix: String,
    #[serde(default)]
    pub system_suffix: String,
    #[serde(default)]
    pub per_stage: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillRouting {
    #[serde(default)]
    pub preferred_providers: Vec<String>,
    #[serde(default)]
    pub avoid_providers: Vec<String>,
    #[serde(default)]
    pub prefer_mode: Option<String>,
    #[serde(default)]
    pub always_verify: bool,
    #[serde(default)]
    pub min_score_threshold: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillTools {
    #[serde(default)]
    pub prefer: Vec<String>,
    #[serde(default)]
    pub suggest_after_code: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillMemory {
    #[serde(default)]
    pub auto_tag: Vec<String>,
    #[serde(default = "default_episodic_limit")]
    pub episodic_limit: usize,
}

fn default_episodic_limit() -> usize {
    3
}

/// Merged directives from all active skills (conflict-resolved)
#[derive(Debug, Clone, Default)]
pub struct MergedDirectives {
    /// Combined system prefix (higher priority first)
    pub system_prefix: String,
    /// Combined system suffix (higher priority first)
    pub system_suffix: String,
    /// Per-stage directives (higher priority wins per stage)
    pub per_stage: HashMap<String, String>,
    /// All auto-tags from active skills
    pub auto_tags: Vec<String>,
}

/// Routing preferences merged from active skills
#[derive(Debug, Clone, Default)]
pub struct RoutingPreferences {
    /// Preferred providers (from highest priority skill)
    pub preferred_providers: Vec<String>,
    /// Providers to avoid (union of all active)
    pub avoid_providers: Vec<String>,
    /// Preferred execution mode (from highest priority skill)
    pub prefer_mode: Option<String>,
    /// Whether to always verify (true if any active skill requires it)
    pub always_verify: bool,
    /// Minimum score threshold (strictest across all active)
    pub min_score_threshold: f32,
}

/// The Steering Engine manages the library of available skills
pub struct SteeringEngine {
    skills_dir: PathBuf,
    skills: HashMap<String, Skill>,
    active: Vec<String>,
}

impl SteeringEngine {
    /// Create a new Steering Engine and load skills from the given directory
    pub async fn new(skills_dir: &Path) -> Result<Self> {
        let mut engine = Self {
            skills_dir: skills_dir.to_path_buf(),
            skills: HashMap::new(),
            active: Vec::new(),
        };

        if skills_dir.exists() && skills_dir.is_dir() {
            engine.load_all_skills().await?;
        } else {
            info!(
                "Skills directory {} does not exist yet.",
                skills_dir.display()
            );
            fs::create_dir_all(skills_dir).await.ok();
        }

        Ok(engine)
    }

    /// Load all `.toml` and `.md` files in the skills directory
    pub async fn load_all_skills(&mut self) -> Result<()> {
        let mut new_skills = HashMap::new();

        let mut entries = fs::read_dir(&self.skills_dir)
            .await
            .context("Failed to read skills directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|s| s.to_str());
            let result = match ext {
                Some("toml") => Self::parse_toml_skill(&path).await,
                Some("md") => Self::parse_md_skill(&path).await,
                _ => continue,
            };

            match result {
                Ok(skill) => {
                    info!("Loaded skill: {} from {}", skill.name, path.display());
                    new_skills.insert(skill.name.to_lowercase(), skill);
                }
                Err(e) => {
                    warn!("Failed to parse skill {}: {}", path.display(), e);
                }
            }
        }

        self.skills = new_skills;
        Ok(())
    }

    /// Reload skills from disk (hot-reload)
    pub async fn reload(&mut self) -> Result<()> {
        let previously_active: Vec<String> = self.active.clone();
        self.load_all_skills().await?;

        // Re-activate previously active skills that still exist
        self.active.clear();
        for id in previously_active {
            if self.skills.contains_key(&id) {
                self.active.push(id);
            }
        }

        info!("Skills reloaded. Active: {:?}", self.active);
        Ok(())
    }

    /// Parse a TOML skill file
    async fn parse_toml_skill(path: &Path) -> Result<Skill> {
        let content = fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let mut skill_file: SkillFile = toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML skill {}", path.display()))?;

        // Enforce max priority for non-built-in skills
        if skill_file.activation.priority > MAX_USER_PRIORITY {
            // Built-in skills (sensitive, local-only) can have priority 100
            // but user skills are capped at 90
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let builtin = ["sensitive", "local-only", "local_only"];
            if !builtin.contains(&stem) {
                warn!(
                    "Skill {} has priority {} > {}, capping to {}",
                    skill_file.meta.id,
                    skill_file.activation.priority,
                    MAX_USER_PRIORITY,
                    MAX_USER_PRIORITY
                );
                skill_file.activation.priority = MAX_USER_PRIORITY;
            }
        }

        // Build system prompt content from directives
        let mut display_content = String::new();
        if !skill_file.directives.system_prefix.is_empty() {
            display_content.push_str(&skill_file.directives.system_prefix);
        }
        if !skill_file.directives.system_suffix.is_empty() {
            if !display_content.is_empty() {
                display_content.push_str("\n\n");
            }
            display_content.push_str(&skill_file.directives.system_suffix);
        }

        Ok(Skill {
            name: skill_file.meta.name.clone(),
            description: skill_file.meta.description.clone(),
            content: display_content,
            file_path: path.to_path_buf(),
            config: Some(skill_file),
        })
    }

    /// Parse a legacy Markdown file containing YAML frontmatter
    async fn parse_md_skill(path: &Path) -> Result<Skill> {
        let file_content = fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read {}", path.display()))?;

        if !file_content.starts_with("---\n") && !file_content.starts_with("---\r\n") {
            return Err(anyhow::anyhow!("Missing YAML frontmatter in skill"));
        }

        let parts: Vec<&str> = file_content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return Err(anyhow::anyhow!(
                "Malformed YAML frontmatter (missing closing ---)"
            ));
        }

        let frontmatter_str = parts[1];
        let content_str = parts[2].trim().to_string();

        let mut name = None;
        let mut description = None;

        for line in frontmatter_str.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("name:") {
                name = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
            } else if let Some(rest) = line.strip_prefix("description:") {
                description = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }

        let name = name.unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap().to_string());
        let description = description.unwrap_or_else(|| "No description provided.".to_string());

        Ok(Skill {
            name,
            description,
            content: content_str,
            file_path: path.to_path_buf(),
            config: None,
        })
    }

    /// Activate a skill by ID (manual activation)
    pub fn activate(&mut self, skill_id: &str) -> Result<()> {
        let key = skill_id.to_lowercase();
        if !self.skills.contains_key(&key) {
            return Err(anyhow::anyhow!("Skill '{}' not found", skill_id));
        }
        if self.active.contains(&key) {
            return Ok(()); // Already active
        }

        // Check conflicts
        if let Some(skill) = self.skills.get(&key) {
            if let Some(ref cfg) = skill.config {
                for conflict in &cfg.activation.conflicts_with {
                    let conflict_key = conflict.to_lowercase();
                    if self.active.contains(&conflict_key) {
                        // Resolve: higher priority wins
                        let conflict_priority = self
                            .skills
                            .get(&conflict_key)
                            .and_then(|s| s.config.as_ref())
                            .map(|c| c.activation.priority)
                            .unwrap_or(0);
                        let new_priority = cfg.activation.priority;

                        if new_priority >= conflict_priority {
                            info!(
                                "Skill '{}' deactivated: conflicts with '{}'",
                                conflict, skill_id
                            );
                            self.active.retain(|a| a != &conflict_key);
                        } else {
                            return Err(anyhow::anyhow!(
                                "Cannot activate '{}': conflicts with '{}' (higher priority)",
                                skill_id,
                                conflict
                            ));
                        }
                    }
                }
            }
        }

        self.active.push(key);
        info!("Skill '{}' activated", skill_id);
        Ok(())
    }

    /// Deactivate a skill
    pub fn deactivate(&mut self, skill_id: &str) {
        let key = skill_id.to_lowercase();
        self.active.retain(|a| a != &key);
        info!("Skill '{}' deactivated", skill_id);
    }

    /// Auto-activate skills based on task content and risk tier
    pub fn auto_activate(&mut self, task_input: &str, risk_tier: u8) {
        let input_lower = task_input.to_lowercase();

        for (key, skill) in &self.skills {
            if self.active.contains(key) {
                continue;
            }
            let cfg = match &skill.config {
                Some(c) => c,
                None => continue,
            };

            for pattern in &cfg.activation.auto_when {
                let should_activate = if let Some(rest) = pattern.strip_prefix("task contains:") {
                    let keywords: Vec<&str> = rest.trim().split('|').map(|s| s.trim()).collect();
                    keywords.iter().any(|kw| input_lower.contains(kw))
                } else if let Some(rest) = pattern.strip_prefix("file type:") {
                    let extensions: Vec<&str> = rest.trim().split('|').map(|s| s.trim()).collect();
                    extensions.iter().any(|ext| input_lower.contains(ext))
                } else if let Some(rest) = pattern.strip_prefix("risk_tier:") {
                    rest.trim()
                        .parse::<u8>()
                        .map(|t| risk_tier >= t)
                        .unwrap_or(false)
                } else {
                    false
                };

                if should_activate {
                    debug!(
                        "Auto-activating skill '{}' (matched: {})",
                        skill.name, pattern
                    );
                    // Use activate to handle conflicts
                    let key_clone = key.clone();
                    // Can't call self.activate here due to borrow, so push directly
                    if !self.active.contains(&key_clone) {
                        self.active.push(key_clone);
                    }
                    break;
                }
            }
        }

        // Resolve conflicts after auto-activation
        self.resolve_conflicts();
    }

    /// Resolve conflicts among active skills
    fn resolve_conflicts(&mut self) {
        let mut to_deactivate = Vec::new();

        for active_key in &self.active {
            if let Some(skill) = self.skills.get(active_key) {
                if let Some(ref cfg) = skill.config {
                    for conflict in &cfg.activation.conflicts_with {
                        let conflict_key = conflict.to_lowercase();
                        if self.active.contains(&conflict_key)
                            && !to_deactivate.contains(&conflict_key)
                        {
                            // Compare priorities
                            let my_priority = cfg.activation.priority;
                            let their_priority = self
                                .skills
                                .get(&conflict_key)
                                .and_then(|s| s.config.as_ref())
                                .map(|c| c.activation.priority)
                                .unwrap_or(0);

                            if my_priority >= their_priority {
                                to_deactivate.push(conflict_key.clone());
                                info!(
                                    "Skill '{}' deactivated: conflicts with '{}'",
                                    conflict, active_key
                                );
                            } else {
                                to_deactivate.push(active_key.clone());
                                info!(
                                    "Skill '{}' deactivated: conflicts with '{}'",
                                    active_key, conflict
                                );
                            }
                        }
                    }
                }
            }
        }

        for key in to_deactivate {
            self.active.retain(|a| a != &key);
        }
    }

    /// Get merged directives from all active skills (conflict-resolved)
    pub fn get_directives(&self) -> MergedDirectives {
        let mut directives = MergedDirectives::default();

        // Sort active skills by priority (highest first)
        let mut active_skills: Vec<&Skill> = self
            .active
            .iter()
            .filter_map(|key| self.skills.get(key))
            .collect();
        active_skills.sort_by(|a, b| {
            let pa = a
                .config
                .as_ref()
                .map(|c| c.activation.priority)
                .unwrap_or(0);
            let pb = b
                .config
                .as_ref()
                .map(|c| c.activation.priority)
                .unwrap_or(0);
            pb.cmp(&pa) // Descending
        });

        for skill in &active_skills {
            if let Some(ref cfg) = skill.config {
                // Concatenate system_prefix (higher priority first)
                if !cfg.directives.system_prefix.is_empty() {
                    if !directives.system_prefix.is_empty() {
                        directives.system_prefix.push('\n');
                    }
                    directives
                        .system_prefix
                        .push_str(&cfg.directives.system_prefix);
                }

                // Concatenate system_suffix
                if !cfg.directives.system_suffix.is_empty() {
                    if !directives.system_suffix.is_empty() {
                        directives.system_suffix.push('\n');
                    }
                    directives
                        .system_suffix
                        .push_str(&cfg.directives.system_suffix);
                }

                // Per-stage: higher priority wins (first insertion wins)
                for (stage, directive) in &cfg.directives.per_stage {
                    directives
                        .per_stage
                        .entry(stage.clone())
                        .or_insert_with(|| directive.clone());
                }

                // Collect all auto-tags
                directives.auto_tags.extend(cfg.memory.auto_tag.clone());
            } else {
                // Legacy .md skill — inject content as system_prefix
                if !skill.content.is_empty() {
                    if !directives.system_prefix.is_empty() {
                        directives.system_prefix.push('\n');
                    }
                    directives
                        .system_prefix
                        .push_str(&format!("# {}\n{}", skill.name, skill.content));
                }
            }
        }

        directives
    }

    /// Get routing preferences from active skills
    pub fn get_routing_prefs(&self) -> RoutingPreferences {
        let mut prefs = RoutingPreferences {
            min_score_threshold: 0.65, // default
            ..Default::default()
        };

        // Sort active skills by priority (highest first)
        let mut active_skills: Vec<&Skill> = self
            .active
            .iter()
            .filter_map(|key| self.skills.get(key))
            .collect();
        active_skills.sort_by(|a, b| {
            let pa = a
                .config
                .as_ref()
                .map(|c| c.activation.priority)
                .unwrap_or(0);
            let pb = b
                .config
                .as_ref()
                .map(|c| c.activation.priority)
                .unwrap_or(0);
            pb.cmp(&pa)
        });

        let mut got_providers = false;
        let mut got_mode = false;

        for skill in &active_skills {
            if let Some(ref cfg) = skill.config {
                // Preferred providers: highest priority wins entirely
                if !got_providers && !cfg.routing.preferred_providers.is_empty() {
                    prefs.preferred_providers = cfg.routing.preferred_providers.clone();
                    got_providers = true;
                }

                // Avoid providers: union of all active
                for p in &cfg.routing.avoid_providers {
                    if !prefs.avoid_providers.contains(p) {
                        prefs.avoid_providers.push(p.clone());
                    }
                }

                // Execution mode: highest priority wins
                if !got_mode {
                    if let Some(ref mode) = cfg.routing.prefer_mode {
                        prefs.prefer_mode = Some(mode.clone());
                        got_mode = true;
                    }
                }

                // Always verify: true if any skill requires it
                if cfg.routing.always_verify {
                    prefs.always_verify = true;
                }

                // Min score: take the strictest (highest)
                if let Some(threshold) = cfg.routing.min_score_threshold {
                    if threshold > prefs.min_score_threshold {
                        prefs.min_score_threshold = threshold;
                    }
                }
            }
        }

        prefs
    }

    /// Retrieve a skill by exact name
    pub fn get_skill(&self, name: &str) -> Option<&Skill> {
        self.skills.get(&name.to_lowercase())
    }

    /// List all loaded skills
    pub fn list_skills(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// List currently active skill IDs
    pub fn active_skills(&self) -> &[String] {
        &self.active
    }

    /// Check if a skill is currently active
    pub fn is_active(&self, skill_id: &str) -> bool {
        self.active.contains(&skill_id.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_md_skill() {
        let dir = tempdir().unwrap();
        let skill_content = r#"---
name: TestSkill
description: A test skill
---
Do something useful."#;
        fs::write(dir.path().join("test.md"), skill_content)
            .await
            .unwrap();

        let engine = SteeringEngine::new(dir.path()).await.unwrap();
        assert_eq!(engine.list_skills().len(), 1);
        let skill = engine.get_skill("TestSkill").unwrap();
        assert_eq!(skill.name, "TestSkill");
        assert_eq!(skill.description, "A test skill");
        assert!(skill.config.is_none());
    }

    #[tokio::test]
    async fn test_load_toml_skill() {
        let dir = tempdir().unwrap();
        let skill_content = r#"
[meta]
id = "careful"
name = "Careful Mode"
description = "Thorough and cautious"
tags = ["quality"]

[activation]
manual = true
priority = 50
conflicts_with = ["fast"]

[directives]
system_prefix = "Be thorough. Verify everything."
system_suffix = "Check edge cases before finalizing."

[directives.per_stage]
Plan = "Break into small steps."
Verify = "Test with edge cases."

[routing]
preferred_providers = ["claude-opus", "claude-sonnet"]
always_verify = true
min_score_threshold = 0.80
"#;
        fs::write(dir.path().join("careful.toml"), skill_content)
            .await
            .unwrap();

        let engine = SteeringEngine::new(dir.path()).await.unwrap();
        assert_eq!(engine.list_skills().len(), 1);

        let skill = engine.get_skill("Careful Mode").unwrap();
        assert_eq!(skill.description, "Thorough and cautious");
        let cfg = skill.config.as_ref().unwrap();
        assert_eq!(cfg.meta.id, "careful");
        assert_eq!(cfg.activation.priority, 50);
        assert_eq!(cfg.activation.conflicts_with, vec!["fast"]);
        assert!(cfg.routing.always_verify);
        assert_eq!(cfg.routing.min_score_threshold, Some(0.80));
    }

    #[tokio::test]
    async fn test_activate_deactivate() {
        let dir = tempdir().unwrap();
        let skill = r#"
[meta]
id = "test"
name = "Test"

[activation]
manual = true
priority = 50
"#;
        fs::write(dir.path().join("test.toml"), skill)
            .await
            .unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();
        assert!(engine.active_skills().is_empty());

        engine.activate("Test").unwrap();
        assert!(engine.is_active("Test"));

        engine.deactivate("Test");
        assert!(!engine.is_active("Test"));
    }

    #[tokio::test]
    async fn test_conflict_resolution() {
        let dir = tempdir().unwrap();

        let careful = r#"
[meta]
id = "careful"
name = "Careful"

[activation]
priority = 60
conflicts_with = ["fast"]

[directives]
system_prefix = "Be careful."
"#;
        let fast = r#"
[meta]
id = "fast"
name = "Fast"

[activation]
priority = 50
conflicts_with = ["careful"]

[directives]
system_prefix = "Be fast."
"#;
        fs::write(dir.path().join("careful.toml"), careful)
            .await
            .unwrap();
        fs::write(dir.path().join("fast.toml"), fast).await.unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();

        // Activate careful first
        engine.activate("Careful").unwrap();
        assert!(engine.is_active("Careful"));

        // Activating fast should fail (careful has higher priority)
        let result = engine.activate("Fast");
        assert!(result.is_err());
        assert!(!engine.is_active("Fast"));
    }

    #[tokio::test]
    async fn test_auto_activation() {
        let dir = tempdir().unwrap();

        let sensitive = r#"
[meta]
id = "sensitive"
name = "Sensitive"

[activation]
priority = 100
auto_when = ["task contains: password|api_key|secret"]

[directives]
system_prefix = "Process locally only."
"#;
        fs::write(dir.path().join("sensitive.toml"), sensitive)
            .await
            .unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();
        assert!(engine.active_skills().is_empty());

        engine.auto_activate("Store my password securely", 0);
        assert!(engine.is_active("sensitive"));
    }

    #[tokio::test]
    async fn test_auto_activation_no_match() {
        let dir = tempdir().unwrap();

        let sensitive = r#"
[meta]
id = "sensitive"
name = "Sensitive"

[activation]
auto_when = ["task contains: password|secret"]
"#;
        fs::write(dir.path().join("sensitive.toml"), sensitive)
            .await
            .unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();
        engine.auto_activate("Hello, how are you?", 0);
        assert!(!engine.is_active("sensitive"));
    }

    #[tokio::test]
    async fn test_merged_directives() {
        let dir = tempdir().unwrap();

        let skill1 = r#"
[meta]
id = "s1"
name = "S1"

[activation]
priority = 60

[directives]
system_prefix = "First."
system_suffix = "Check 1."

[directives.per_stage]
Plan = "Plan carefully."
"#;
        let skill2 = r#"
[meta]
id = "s2"
name = "S2"

[activation]
priority = 40

[directives]
system_prefix = "Second."

[directives.per_stage]
Plan = "Plan quickly."
Execute = "Execute fast."
"#;
        fs::write(dir.path().join("s1.toml"), skill1).await.unwrap();
        fs::write(dir.path().join("s2.toml"), skill2).await.unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();
        engine.activate("S1").unwrap();
        engine.activate("S2").unwrap();

        let directives = engine.get_directives();

        // S1 has higher priority, its prefix comes first
        assert!(directives.system_prefix.starts_with("First."));
        assert!(directives.system_prefix.contains("Second."));
        assert_eq!(directives.system_suffix, "Check 1.");

        // Per-stage: S1 wins for Plan (higher priority, inserted first)
        assert_eq!(directives.per_stage.get("Plan").unwrap(), "Plan carefully.");
        // S2 provides Execute
        assert_eq!(
            directives.per_stage.get("Execute").unwrap(),
            "Execute fast."
        );
    }

    #[tokio::test]
    async fn test_routing_prefs_strictest_score() {
        let dir = tempdir().unwrap();

        let s1 = r#"
[meta]
id = "s1"
name = "S1"

[routing]
min_score_threshold = 0.75
always_verify = false
"#;
        let s2 = r#"
[meta]
id = "s2"
name = "S2"

[routing]
min_score_threshold = 0.85
always_verify = true
"#;
        fs::write(dir.path().join("s1.toml"), s1).await.unwrap();
        fs::write(dir.path().join("s2.toml"), s2).await.unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();
        engine.activate("S1").unwrap();
        engine.activate("S2").unwrap();

        let prefs = engine.get_routing_prefs();
        // Strictest score wins
        assert_eq!(prefs.min_score_threshold, 0.85);
        // Any skill requiring verify → true
        assert!(prefs.always_verify);
    }

    #[tokio::test]
    async fn test_priority_capping() {
        let dir = tempdir().unwrap();

        let skill = r#"
[meta]
id = "user-skill"
name = "UserSkill"

[activation]
priority = 200

[directives]
system_prefix = "I am important."
"#;
        fs::write(dir.path().join("user-skill.toml"), skill)
            .await
            .unwrap();

        let engine = SteeringEngine::new(dir.path()).await.unwrap();
        let s = engine.get_skill("UserSkill").unwrap();
        // Priority should be capped to 90
        assert_eq!(
            s.config.as_ref().unwrap().activation.priority,
            MAX_USER_PRIORITY
        );
    }

    #[tokio::test]
    async fn test_reload() {
        let dir = tempdir().unwrap();

        let skill = r#"
[meta]
id = "s1"
name = "S1"

[activation]
manual = true
"#;
        fs::write(dir.path().join("s1.toml"), skill).await.unwrap();

        let mut engine = SteeringEngine::new(dir.path()).await.unwrap();
        engine.activate("S1").unwrap();
        assert!(engine.is_active("s1"));

        // Add another skill file
        let skill2 = r#"
[meta]
id = "s2"
name = "S2"
"#;
        fs::write(dir.path().join("s2.toml"), skill2).await.unwrap();

        engine.reload().await.unwrap();

        // S1 should still be active
        assert!(engine.is_active("s1"));
        // S2 should now be loadable
        assert!(engine.get_skill("S2").is_some());
        assert_eq!(engine.list_skills().len(), 2);
    }

    #[tokio::test]
    async fn test_empty_dir() {
        let dir = tempdir().unwrap();
        let engine = SteeringEngine::new(dir.path()).await.unwrap();
        assert!(engine.list_skills().is_empty());
        assert!(engine.active_skills().is_empty());
    }

    #[tokio::test]
    async fn test_nonexistent_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nonexistent");
        let engine = SteeringEngine::new(&missing).await.unwrap();
        assert!(engine.list_skills().is_empty());
        // Directory should have been created
        assert!(missing.exists());
    }
}
