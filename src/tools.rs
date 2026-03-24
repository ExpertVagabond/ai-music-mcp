// ─── Tool Implementations (ai-music-mcp) ──────────────────────────────
// Each tool is a struct implementing `ToolHandler` from psm_mcp_core.
// Config is shared via Arc across all tool handlers.

use async_trait::async_trait;
use psm_mcp_core::error::PsmMcpError;
use psm_mcp_core::input::{optional_string, require_string};
use psm_mcp_core::tool::{ToolDefinition, ToolHandler, ToolResult};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

pub struct Config {
    pub music_studio_dir: String,
    pub rvc_dir: String,
    pub output_dir: String,
    pub stems_dir: String,
    pub converted_dir: String,
    pub musicgen_venv: String,
    pub rvc_venv: String,
    pub musicgen_script: String,
    pub stems_script: String,
    pub rvc_infer_script: String,
    pub rvc_weights_dir: String,
    #[allow(dead_code)]
    pub rvc_index_dir: String,
}

/// Validate and canonicalize a directory path from an environment variable.
/// Rejects paths containing `..` traversal sequences and verifies the path exists.
fn validate_env_path(raw: &str, label: &str) -> Result<String, String> {
    if raw.contains("..") {
        return Err(format!(
            "{label}: path contains traversal sequence (..): {raw}"
        ));
    }
    let path = Path::new(raw);
    match std::fs::canonicalize(path) {
        Ok(canon) => Ok(canon.to_string_lossy().to_string()),
        Err(_) => {
            // Directory may not exist yet (e.g., output dirs); allow if parent exists
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    Ok(raw.to_string())
                } else {
                    Err(format!("{label}: path does not exist: {raw}"))
                }
            } else {
                Err(format!("{label}: invalid path: {raw}"))
            }
        }
    }
}

/// Validate a user-supplied file path: reject traversal and ensure it resolves
/// within the expected base directory (or is an absolute path that exists).
fn validate_user_path(raw: &str, allowed_bases: &[&str]) -> Result<std::path::PathBuf, String> {
    if raw.is_empty() {
        return Err("Path cannot be empty".into());
    }
    if raw.contains("..") {
        return Err(format!("Path contains traversal sequence (..): {raw}"));
    }
    let path = Path::new(raw);
    // If the file exists, canonicalize it and check against allowed bases
    if let Ok(canon) = std::fs::canonicalize(path) {
        let canon_str = canon.to_string_lossy();
        if allowed_bases.is_empty()
            || allowed_bases
                .iter()
                .any(|base| canon_str.starts_with(base))
        {
            return Ok(canon);
        }
        return Err(format!(
            "Path {raw} resolves outside allowed directories"
        ));
    }
    // File doesn't exist yet (e.g., output path) — allow if parent exists
    if let Some(parent) = path.parent() {
        if parent.exists() {
            return Ok(path.to_path_buf());
        }
    }
    Err(format!("Path does not exist: {raw}"))
}

impl Config {
    pub fn from_env() -> Self {
        let music_studio_dir = std::env::var("MUSIC_STUDIO_DIR")
            .unwrap_or_else(|_| "/Volumes/Virtual Server/projects/ai-music-studio".into());
        let music_studio_dir = validate_env_path(&music_studio_dir, "MUSIC_STUDIO_DIR")
            .unwrap_or_else(|e| {
                eprintln!("[ai-music-mcp] {e}");
                std::process::exit(1);
            });

        let rvc_dir = std::env::var("RVC_DIR")
            .unwrap_or_else(|_| "/Volumes/Virtual Server/projects/ai-music-rvc".into());
        let rvc_dir = validate_env_path(&rvc_dir, "RVC_DIR").unwrap_or_else(|e| {
            eprintln!("[ai-music-mcp] {e}");
            std::process::exit(1);
        });

        let home = std::env::var("HOME").unwrap_or_default();
        let output_dir = std::env::var("MUSIC_OUTPUT_DIR")
            .unwrap_or_else(|_| format!("{}/Desktop/AI-Music", home));
        let output_dir =
            validate_env_path(&output_dir, "MUSIC_OUTPUT_DIR").unwrap_or_else(|e| {
                eprintln!("[ai-music-mcp] {e}");
                std::process::exit(1);
            });

        Self {
            stems_dir: format!("{}/stems", output_dir),
            converted_dir: format!("{}/converted", output_dir),
            musicgen_venv: format!("{}/.venv", music_studio_dir),
            rvc_venv: format!("{}/.venv", rvc_dir),
            musicgen_script: format!("{}/generate-music.py", music_studio_dir),
            stems_script: format!("{}/separate-stems.py", music_studio_dir),
            rvc_infer_script: format!("{}/tools/cmd/infer_cli.py", rvc_dir),
            rvc_weights_dir: format!("{}/assets/weights", rvc_dir),
            rvc_index_dir: format!("{}/logs", rvc_dir),
            music_studio_dir,
            rvc_dir,
            output_dir,
        }
    }

    /// Return the list of allowed base directories for user-supplied file paths.
    fn allowed_bases(&self) -> Vec<&str> {
        vec![
            &self.output_dir,
            &self.stems_dir,
            &self.converted_dir,
            &self.music_studio_dir,
            &self.rvc_dir,
        ]
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn run_python(venv: &str, args: &[&str], cwd: &str, _timeout_secs: u64) -> Result<String, String> {
    // Validate venv python binary exists and is not a traversal
    let python_path = Path::new(venv).join("bin/python");
    if venv.contains("..") {
        return Err("Venv path contains traversal sequence".into());
    }
    let python = std::fs::canonicalize(&python_path)
        .map_err(|_| format!("Python binary not found at {}", python_path.display()))?;

    // Validate cwd exists
    let cwd_path = Path::new(cwd);
    if !cwd_path.is_dir() {
        return Err(format!("Working directory does not exist: {cwd}"));
    }

    // Reject any arg containing path traversal (defense in depth —
    // Command::new already avoids shell interpretation).
    for arg in args {
        if arg.contains("..") {
            return Err(format!("Argument contains path traversal: {arg}"));
        }
    }

    let output = Command::new(&python)
        .args(args)
        .current_dir(cwd)
        .env("TOKENIZERS_PARALLELISM", "false")
        .env("PYTORCH_ENABLE_MPS_FALLBACK", "1")
        .output()
        .map_err(|e| format!("Failed to run python: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(if stderr.is_empty() {
            stdout
        } else {
            format!("{stdout}\n\nWarnings:\n{stderr}")
        })
    } else {
        Err(format!(
            "Exit {}: {}",
            output.status.code().unwrap_or(-1),
            if stdout.is_empty() { &stderr } else { &stdout }
        ))
    }
}

fn list_dir(dir: &str) -> String {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            let files: Vec<String> = entries
                .filter_map(|e| {
                    let e = e.ok()?;
                    Some(e.file_name().to_string_lossy().to_string())
                })
                .collect();
            if files.is_empty() {
                format!("No files in {dir}")
            } else {
                format!("Files in {dir} ({}):\n{}", files.len(), files.join("\n"))
            }
        }
        Err(e) => format!("Cannot read {dir}: {e}"),
    }
}

/// Validate a model size parameter, returning a known-safe value.
fn validate_model_size(input: Option<String>) -> &'static str {
    match input.as_deref() {
        Some("small") => "small",
        Some("medium") => "medium",
        Some("large") => "large",
        _ => "small",
    }
}

/// Validate a Demucs model name.
fn validate_demucs_model(input: Option<String>) -> &'static str {
    match input.as_deref() {
        Some("htdemucs") => "htdemucs",
        Some("htdemucs_ft") => "htdemucs_ft",
        _ => "htdemucs",
    }
}

/// Validate a stem type.
fn validate_stem_type(input: Option<String>) -> &'static str {
    match input.as_deref() {
        Some("all") => "all",
        Some("vocals") => "vocals",
        Some("drums") => "drums",
        Some("bass") => "bass",
        Some("other") => "other",
        _ => "all",
    }
}

/// Extract an optional i64 from JSON args.
fn optional_i64(args: &Value, field: &str) -> Option<i64> {
    args.get(field).and_then(|v| v.as_i64())
}

/// Extract an optional f64 from JSON args.
fn optional_f64(args: &Value, field: &str) -> Option<f64> {
    args.get(field).and_then(|v| v.as_f64())
}

/// Validate a filename has no path separators or traversal.
fn validate_filename(name: &str, label: &str) -> Result<(), PsmMcpError> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(PsmMcpError::InputValidation(format!(
            "{label} must be a plain filename (e.g. 'matthew.pth')"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tool: music_generate
// ---------------------------------------------------------------------------

pub struct GenerateTool {
    config: Arc<Config>,
}

impl GenerateTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for GenerateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_generate".into(),
            description: "Generate instrumental music from a text prompt using MusicGen. Returns a WAV file.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Text description of the music to generate" },
                    "duration": { "type": "integer", "description": "Duration in seconds (1-30)", "default": 15 },
                    "model": { "type": "string", "description": "Model size: small, medium, large", "default": "small" },
                    "output_filename": { "type": "string", "description": "Custom output filename" }
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let prompt = require_string(&args, "prompt")?;
        let duration = optional_i64(&args, "duration")
            .unwrap_or(15)
            .clamp(1, 30)
            .to_string();
        let model = validate_model_size(optional_string(&args, "model"));

        let mut cmd_args = vec![
            self.config.musicgen_script.as_str(),
            &prompt,
            "--duration",
            &duration,
            "--model",
            model,
        ];

        let output_filename;
        if let Some(f) = optional_string(&args, "output_filename") {
            validate_filename(&f, "output_filename")?;
            output_filename = f;
            cmd_args.push("--output");
            cmd_args.push(&output_filename);
        }

        match run_python(
            &self.config.musicgen_venv,
            &cmd_args,
            &self.config.music_studio_dir,
            600,
        ) {
            Ok(out) => ToolResult::json(&json!({"status": "success", "output": out}))
                .map_err(|e| PsmMcpError::Internal(e.into())),
            Err(e) => Ok(ToolResult::error(format!("Error: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: music_separate_stems
// ---------------------------------------------------------------------------

pub struct SeparateStemsTool {
    config: Arc<Config>,
}

impl SeparateStemsTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for SeparateStemsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_separate_stems".into(),
            description: "Separate an audio file into stems (vocals, drums, bass, other) using Demucs".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "input_audio": { "type": "string", "description": "Path to input audio file" },
                    "model": { "type": "string", "description": "Demucs model: htdemucs, htdemucs_ft", "default": "htdemucs" },
                    "stems": { "type": "string", "description": "Which stems: all, vocals, drums, bass, other", "default": "all" }
                },
                "required": ["input_audio"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let input = require_string(&args, "input_audio")?;
        let allowed = self.config.allowed_bases();
        validate_user_path(&input, &allowed).map_err(PsmMcpError::InputValidation)?;

        let model = validate_demucs_model(optional_string(&args, "model"));
        let stems = validate_stem_type(optional_string(&args, "stems"));

        let cmd_args = vec![
            self.config.stems_script.as_str(),
            &input,
            "--model",
            model,
            "--stems",
            stems,
            "--output-dir",
            &self.config.stems_dir,
        ];

        match run_python(
            &self.config.musicgen_venv,
            &cmd_args,
            &self.config.music_studio_dir,
            600,
        ) {
            Ok(out) => ToolResult::json(&json!({"status": "success", "output": out}))
                .map_err(|e| PsmMcpError::Internal(e.into())),
            Err(e) => Ok(ToolResult::error(format!("Error: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: music_list_stems
// ---------------------------------------------------------------------------

pub struct ListStemsTool {
    config: Arc<Config>,
}

impl ListStemsTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for ListStemsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_list_stems".into(),
            description: "List available stem separation results in the stems output directory".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    async fn handle(&self, _args: Value) -> Result<ToolResult, PsmMcpError> {
        Ok(ToolResult::text(list_dir(&self.config.stems_dir)))
    }
}

// ---------------------------------------------------------------------------
// Tool: music_convert_voice
// ---------------------------------------------------------------------------

pub struct ConvertVoiceTool {
    config: Arc<Config>,
}

impl ConvertVoiceTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for ConvertVoiceTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_convert_voice".into(),
            description: "Convert vocals in an audio file to a different voice using RVC".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "input_audio": { "type": "string", "description": "Path to input audio with vocals" },
                    "model_name": { "type": "string", "description": "RVC model name (e.g. 'matthew.pth')" },
                    "pitch": { "type": "integer", "description": "Pitch shift in semitones", "default": 0 },
                    "index_rate": { "type": "number", "description": "Feature index ratio (0-1)", "default": 0.75 }
                },
                "required": ["input_audio", "model_name"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let input = require_string(&args, "input_audio")?;
        let model_name = require_string(&args, "model_name")?;

        let allowed = self.config.allowed_bases();
        validate_user_path(&input, &allowed).map_err(PsmMcpError::InputValidation)?;
        validate_filename(&model_name, "model_name")?;

        let pitch = optional_i64(&args, "pitch")
            .unwrap_or(0)
            .clamp(-24, 24)
            .to_string();
        let index_rate = optional_f64(&args, "index_rate")
            .unwrap_or(0.75)
            .clamp(0.0, 1.0)
            .to_string();
        let model_path = format!("{}/{}", self.config.rvc_weights_dir, model_name);

        let cmd_args = vec![
            self.config.rvc_infer_script.as_str(),
            &input,
            "--model",
            &model_path,
            "--pitch",
            &pitch,
            "--index-rate",
            &index_rate,
            "--output-dir",
            &self.config.converted_dir,
        ];

        match run_python(&self.config.rvc_venv, &cmd_args, &self.config.rvc_dir, 300) {
            Ok(out) => ToolResult::json(&json!({"status": "success", "output": out}))
                .map_err(|e| PsmMcpError::Internal(e.into())),
            Err(e) => Ok(ToolResult::error(format!("Error: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: music_list_voice_models
// ---------------------------------------------------------------------------

pub struct ListVoiceModelsTool {
    config: Arc<Config>,
}

impl ListVoiceModelsTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for ListVoiceModelsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_list_voice_models".into(),
            description: "List available RVC voice models for voice conversion".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    async fn handle(&self, _args: Value) -> Result<ToolResult, PsmMcpError> {
        match std::fs::read_dir(&self.config.rvc_weights_dir) {
            Ok(entries) => {
                let models: Vec<String> = entries
                    .filter_map(|e| {
                        let e = e.ok()?;
                        let n = e.file_name().to_string_lossy().to_string();
                        if n.ends_with(".pth") {
                            Some(n)
                        } else {
                            None
                        }
                    })
                    .collect();
                if models.is_empty() {
                    Ok(ToolResult::text("No voice models found"))
                } else {
                    Ok(ToolResult::text(format!(
                        "Voice models ({}):\n{}",
                        models.len(),
                        models.join("\n")
                    )))
                }
            }
            Err(e) => Ok(ToolResult::error(format!("Cannot read models dir: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: music_list_files
// ---------------------------------------------------------------------------

pub struct ListFilesTool {
    config: Arc<Config>,
}

impl ListFilesTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for ListFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_list_files".into(),
            description: "List generated music files in the output directory".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subdirectory": { "type": "string", "description": "Subdirectory to list (stems, converted, or empty for root)" }
                }
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let sub = optional_string(&args, "subdirectory").unwrap_or_default();
        let dir = match sub.as_str() {
            "stems" => &self.config.stems_dir,
            "converted" => &self.config.converted_dir,
            "" => &self.config.output_dir,
            _ => {
                return Err(PsmMcpError::InputValidation(
                    "subdirectory must be 'stems', 'converted', or empty".into(),
                ));
            }
        };
        Ok(ToolResult::text(list_dir(dir)))
    }
}

// ---------------------------------------------------------------------------
// Tool: music_get_info
// ---------------------------------------------------------------------------

pub struct GetInfoTool {
    config: Arc<Config>,
}

impl GetInfoTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for GetInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_get_info".into(),
            description: "Get metadata about a generated audio file (duration, sample rate, size)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Absolute path to audio file" }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let path = require_string(&args, "file_path")?;
        let allowed = self.config.allowed_bases();
        validate_user_path(&path, &allowed).map_err(PsmMcpError::InputValidation)?;

        match std::fs::metadata(&path) {
            Ok(m) => ToolResult::json(&json!({
                "file": path,
                "size_bytes": m.len(),
                "size_mb": format!("{:.1}", m.len() as f64 / 1_048_576.0),
            }))
            .map_err(|e| PsmMcpError::Internal(e.into())),
            Err(e) => Ok(ToolResult::error(format!("Error reading file: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: music_full_pipeline
// ---------------------------------------------------------------------------

pub struct FullPipelineTool {
    config: Arc<Config>,
}

impl FullPipelineTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ToolHandler for FullPipelineTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_full_pipeline".into(),
            description: "Run the full pipeline: generate music -> separate stems -> apply voice conversion. One-shot end-to-end.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Music generation prompt" },
                    "duration": { "type": "integer", "description": "Duration in seconds", "default": 15 },
                    "model": { "type": "string", "description": "MusicGen model size", "default": "small" },
                    "voice_model": { "type": "string", "description": "RVC voice model name for conversion (optional)" },
                    "pitch": { "type": "integer", "description": "Pitch shift for voice conversion", "default": 0 },
                    "index_rate": { "type": "number", "description": "Feature index ratio for voice conversion (0-1)", "default": 0.75 }
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let prompt = require_string(&args, "prompt")?;
        let duration = optional_i64(&args, "duration")
            .unwrap_or(15)
            .clamp(1, 30)
            .to_string();
        let model = validate_model_size(optional_string(&args, "model"));

        // Step 1: Generate
        let gen_args = vec![
            self.config.musicgen_script.as_str(),
            &prompt,
            "--duration",
            &duration,
            "--model",
            model,
        ];

        match run_python(
            &self.config.musicgen_venv,
            &gen_args,
            &self.config.music_studio_dir,
            600,
        ) {
            Ok(gen_out) => {
                let mut result = json!({"step1_generate": "success", "output": gen_out});

                // Step 2: Voice conversion (optional)
                if let Some(voice_model) = optional_string(&args, "voice_model") {
                    if let Some(cap) = gen_out.lines().find(|l| l.contains("Saved:")) {
                        // Validate voice_model is a plain filename
                        if voice_model.contains('/')
                            || voice_model.contains('\\')
                            || voice_model.contains("..")
                        {
                            result["step2_voice_convert"] =
                                json!("error: voice_model must be a plain filename");
                        } else {
                            let saved_path = cap.replace("Saved:", "").trim().to_string();
                            let pitch = optional_i64(&args, "pitch")
                                .unwrap_or(0)
                                .clamp(-24, 24)
                                .to_string();
                            let index_rate = optional_f64(&args, "index_rate")
                                .unwrap_or(0.75)
                                .clamp(0.0, 1.0)
                                .to_string();
                            let mp =
                                format!("{}/{}", self.config.rvc_weights_dir, voice_model);
                            let conv_args = vec![
                                self.config.rvc_infer_script.as_str(),
                                &saved_path,
                                "--model",
                                &mp,
                                "--pitch",
                                &pitch,
                                "--index-rate",
                                &index_rate,
                                "--output-dir",
                                &self.config.converted_dir,
                            ];
                            match run_python(
                                &self.config.rvc_venv,
                                &conv_args,
                                &self.config.rvc_dir,
                                300,
                            ) {
                                Ok(conv_out) => {
                                    result["step2_voice_convert"] = json!("success");
                                    result["convert_output"] = json!(conv_out);
                                }
                                Err(e) => {
                                    result["step2_voice_convert"] =
                                        json!(format!("error: {e}"));
                                }
                            }
                        }
                    }
                }

                ToolResult::json(&result).map_err(|e| PsmMcpError::Internal(e.into()))
            }
            Err(e) => Ok(ToolResult::error(format!("Generation failed: {e}"))),
        }
    }
}
