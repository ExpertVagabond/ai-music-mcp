use serde::Deserialize;
use serde_json::{Value, json};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Security: Constants & limits
// ---------------------------------------------------------------------------

/// Maximum JSON-RPC request size (1 MB). Prevents memory exhaustion from
/// oversized payloads on stdin.
const MAX_REQUEST_SIZE: usize = 1_024 * 1_024;

/// Maximum string parameter length (8 KB).
const MAX_PARAM_LEN: usize = 8_192;

/// Maximum allowed command arguments to prevent arg-list attacks.
const MAX_ARGS: usize = 64;

/// Validate a string parameter: reject null bytes, enforce length.
fn validate_string_param(value: &str, label: &str) -> Result<(), String> {
    if value.len() > MAX_PARAM_LEN {
        return Err(format!("{label}: exceeds maximum length of {MAX_PARAM_LEN}"));
    }
    if value.contains('\0') {
        return Err(format!("{label}: contains null bytes"));
    }
    Ok(())
}

/// Redact internal paths from error messages for safe external display.
fn redact_error(msg: &str) -> String {
    let mut s = msg.to_string();
    // Redact absolute paths
    while let Some(start) = s.find("/Volumes/") {
        if let Some(end) = s[start..].find(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
            s.replace_range(start..start + end, "[redacted-path]");
        } else {
            s.replace_range(start.., "[redacted-path]");
            break;
        }
    }
    while let Some(start) = s.find("/Users/") {
        if let Some(end) = s[start..].find(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
            s.replace_range(start..start + end, "[redacted-path]");
        } else {
            s.replace_range(start.., "[redacted-path]");
            break;
        }
    }
    if s.len() > 500 {
        s.truncate(500);
        s.push_str("... (truncated)");
    }
    s
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

struct Config {
    music_studio_dir: String,
    rvc_dir: String,
    output_dir: String,
    stems_dir: String,
    converted_dir: String,
    musicgen_venv: String,
    rvc_venv: String,
    musicgen_script: String,
    stems_script: String,
    rvc_infer_script: String,
    rvc_weights_dir: String,
    rvc_index_dir: String,
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
fn validate_user_path(raw: &str, allowed_bases: &[&str]) -> Result<PathBuf, String> {
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
    if let Some(parent) = path.parent()
        && parent.exists()
    {
        return Ok(path.to_path_buf());
    }
    Err(format!("Path does not exist: {raw}"))
}

impl Config {
    fn from_env() -> Self {
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
}

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

    // Reject any arg containing shell metacharacters (defense in depth —
    // Command::new already avoids shell interpretation, but this blocks
    // attempts to sneak in flags like --import or path traversals in args).
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

fn tool_definitions() -> Value {
    json!([
        {
            "name": "music_generate",
            "description": "Generate instrumental music from a text prompt using MusicGen. Returns a WAV file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Text description of the music to generate" },
                    "duration": { "type": "integer", "description": "Duration in seconds (1-30)", "default": 15 },
                    "model": { "type": "string", "description": "Model size: small, medium, large", "default": "small" },
                    "output_filename": { "type": "string", "description": "Custom output filename" }
                },
                "required": ["prompt"]
            }
        },
        {
            "name": "music_separate_stems",
            "description": "Separate an audio file into stems (vocals, drums, bass, other) using Demucs",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input_audio": { "type": "string", "description": "Path to input audio file" },
                    "model": { "type": "string", "description": "Demucs model: htdemucs, htdemucs_ft", "default": "htdemucs" },
                    "stems": { "type": "string", "description": "Which stems: all, vocals, drums, bass, other", "default": "all" }
                },
                "required": ["input_audio"]
            }
        },
        {
            "name": "music_list_stems",
            "description": "List available stem separation results in the stems output directory",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "music_convert_voice",
            "description": "Convert vocals in an audio file to a different voice using RVC",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input_audio": { "type": "string", "description": "Path to input audio with vocals" },
                    "model_name": { "type": "string", "description": "RVC model name (e.g. 'matthew.pth')" },
                    "pitch": { "type": "integer", "description": "Pitch shift in semitones", "default": 0 },
                    "index_rate": { "type": "number", "description": "Feature index ratio (0-1)", "default": 0.75 }
                },
                "required": ["input_audio", "model_name"]
            }
        },
        {
            "name": "music_list_voice_models",
            "description": "List available RVC voice models for voice conversion",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "music_list_files",
            "description": "List generated music files in the output directory",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "subdirectory": { "type": "string", "description": "Subdirectory to list (stems, converted, or empty for root)" }
                }
            }
        },
        {
            "name": "music_get_info",
            "description": "Get metadata about a generated audio file (duration, sample rate, size)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Absolute path to audio file" }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "music_full_pipeline",
            "description": "Run the full pipeline: generate music -> separate stems -> apply voice conversion. One-shot end-to-end.",
            "inputSchema": {
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
            }
        }
    ])
}

fn call_tool(name: &str, args: &Value, config: &Config) -> Value {
    // Allowed base directories for user-supplied file paths
    let allowed_bases: Vec<&str> = vec![
        &config.output_dir,
        &config.stems_dir,
        &config.converted_dir,
        &config.music_studio_dir,
        &config.rvc_dir,
    ];

    let text = match name {
        "music_generate" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            let duration = args["duration"].as_i64().unwrap_or(15).clamp(1, 30).to_string();
            let model = match args["model"].as_str().unwrap_or("small") {
                "small" | "medium" | "large" => args["model"].as_str().unwrap_or("small"),
                _ => "small",
            };
            let mut cmd_args = vec![
                config.musicgen_script.as_str(),
                prompt,
                "--duration",
                &duration,
                "--model",
                model,
            ];
            let output_filename;
            if let Some(f) = args["output_filename"].as_str() {
                // Reject path separators and traversal in filename
                if f.contains('/') || f.contains('\\') || f.contains("..") {
                    return json!({"content":[{"type":"text","text":"Error: output_filename must be a plain filename, not a path"}]});
                }
                output_filename = f.to_string();
                cmd_args.push("--output");
                cmd_args.push(&output_filename);
            }
            match run_python(
                &config.musicgen_venv,
                &cmd_args,
                &config.music_studio_dir,
                600,
            ) {
                Ok(out) => serde_json::to_string_pretty(&json!({"status":"success","output":out}))
                    .unwrap_or_default()
                    .to_string(),
                Err(e) => format!("Error: {e}"),
            }
        }
        "music_separate_stems" => {
            let input = args["input_audio"].as_str().unwrap_or("");
            if let Err(e) = validate_user_path(input, &allowed_bases) {
                return json!({"content":[{"type":"text","text":format!("Error: {e}")}]});
            }
            let model = match args["model"].as_str().unwrap_or("htdemucs") {
                "htdemucs" | "htdemucs_ft" => args["model"].as_str().unwrap_or("htdemucs"),
                _ => "htdemucs",
            };
            let stems = match args["stems"].as_str().unwrap_or("all") {
                "all" | "vocals" | "drums" | "bass" | "other" => {
                    args["stems"].as_str().unwrap_or("all")
                }
                _ => "all",
            };
            let cmd_args = vec![
                config.stems_script.as_str(),
                input,
                "--model",
                model,
                "--stems",
                stems,
                "--output-dir",
                &config.stems_dir,
            ];
            match run_python(
                &config.musicgen_venv,
                &cmd_args,
                &config.music_studio_dir,
                600,
            ) {
                Ok(out) => serde_json::to_string_pretty(&json!({"status":"success","output":out}))
                    .unwrap_or_default()
                    .to_string(),
                Err(e) => format!("Error: {e}"),
            }
        }
        "music_list_stems" => list_dir(&config.stems_dir),
        "music_convert_voice" => {
            let input = args["input_audio"].as_str().unwrap_or("");
            if let Err(e) = validate_user_path(input, &allowed_bases) {
                return json!({"content":[{"type":"text","text":format!("Error: {e}")}]});
            }
            let model_name = args["model_name"].as_str().unwrap_or("");
            // model_name must be a plain filename (no path separators or traversal)
            if model_name.contains('/')
                || model_name.contains('\\')
                || model_name.contains("..")
                || model_name.is_empty()
            {
                return json!({"content":[{"type":"text","text":"Error: model_name must be a plain filename (e.g. 'matthew.pth')"}]});
            }
            let pitch = args["pitch"].as_i64().unwrap_or(0).clamp(-24, 24).to_string();
            let index_rate = args["index_rate"]
                .as_f64()
                .unwrap_or(0.75)
                .clamp(0.0, 1.0)
                .to_string();
            let model_path = format!("{}/{}", config.rvc_weights_dir, model_name);
            let cmd_args = vec![
                config.rvc_infer_script.as_str(),
                input,
                "--model",
                &model_path,
                "--pitch",
                &pitch,
                "--index-rate",
                &index_rate,
                "--output-dir",
                &config.converted_dir,
            ];
            match run_python(&config.rvc_venv, &cmd_args, &config.rvc_dir, 300) {
                Ok(out) => serde_json::to_string_pretty(&json!({"status":"success","output":out}))
                    .unwrap_or_default()
                    .to_string(),
                Err(e) => format!("Error: {e}"),
            }
        }
        "music_list_voice_models" => match std::fs::read_dir(&config.rvc_weights_dir) {
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
                    "No voice models found".into()
                } else {
                    format!("Voice models ({}):\n{}", models.len(), models.join("\n"))
                }
            }
            Err(e) => format!("Cannot read models dir: {e}"),
        },
        "music_list_files" => {
            // Only allow known subdirectory names — no arbitrary path input
            let sub = args["subdirectory"].as_str().unwrap_or("");
            let dir = match sub {
                "stems" => &config.stems_dir,
                "converted" => &config.converted_dir,
                "" => &config.output_dir,
                _ => {
                    return json!({"content":[{"type":"text","text":"Error: subdirectory must be 'stems', 'converted', or empty"}]});
                }
            };
            list_dir(dir)
        }
        "music_get_info" => {
            let path = args["file_path"].as_str().unwrap_or("");
            if let Err(e) = validate_user_path(path, &allowed_bases) {
                return json!({"content":[{"type":"text","text":format!("Error: {e}")}]});
            }
            match std::fs::metadata(path) {
                Ok(m) => serde_json::to_string_pretty(&json!({
                    "file": path,
                    "size_bytes": m.len(),
                    "size_mb": format!("{:.1}", m.len() as f64 / 1_048_576.0),
                }))
                .unwrap_or_default()
                .to_string(),
                Err(e) => format!("Error reading file: {e}"),
            }
        }
        "music_full_pipeline" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            let duration = args["duration"].as_i64().unwrap_or(15).clamp(1, 30).to_string();
            let model = match args["model"].as_str().unwrap_or("small") {
                "small" | "medium" | "large" => args["model"].as_str().unwrap_or("small"),
                _ => "small",
            };
            // Step 1: Generate
            let gen_args = vec![
                config.musicgen_script.as_str(),
                prompt,
                "--duration",
                &duration,
                "--model",
                model,
            ];
            match run_python(
                &config.musicgen_venv,
                &gen_args,
                &config.music_studio_dir,
                600,
            ) {
                Ok(gen_out) => {
                    let mut result = json!({"step1_generate": "success", "output": gen_out});
                    // Try to extract file path from output for voice conversion
                    if let Some(voice_model) = args["voice_model"].as_str()
                        && let Some(cap) = gen_out.lines().find(|l| l.contains("Saved:"))
                    {
                        // Validate voice_model is a plain filename
                        if voice_model.contains('/')
                            || voice_model.contains('\\')
                            || voice_model.contains("..")
                        {
                            result["step2_voice_convert"] =
                                json!("error: voice_model must be a plain filename");
                        } else {
                            let saved_path = cap.replace("Saved:", "").trim().to_string();
                            let pitch =
                                args["pitch"].as_i64().unwrap_or(0).clamp(-24, 24).to_string();
                            let index_rate = args["index_rate"]
                                .as_f64()
                                .unwrap_or(0.75)
                                .clamp(0.0, 1.0)
                                .to_string();
                            let mp = format!("{}/{}", config.rvc_weights_dir, voice_model);
                            let conv_args = vec![
                                config.rvc_infer_script.as_str(),
                                &saved_path,
                                "--model",
                                &mp,
                                "--pitch",
                                &pitch,
                                "--index-rate",
                                &index_rate,
                                "--output-dir",
                                &config.converted_dir,
                            ];
                            match run_python(&config.rvc_venv, &conv_args, &config.rvc_dir, 300) {
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
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                }
                Err(e) => format!("Generation failed: {e}"),
            }
        }
        _ => format!("Unknown tool: {name}"),
    };
    json!({"content":[{"type":"text","text":text}]})
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

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .init();
    let config = Config::from_env();
    eprintln!("[ai-music-mcp] Starting with 8 tools");
    eprintln!(
        "[ai-music-mcp] MusicGen: [configured], RVC: [configured], Output: [configured]"
    );
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        // Security: reject oversized requests
        if line.len() > MAX_REQUEST_SIZE {
            eprintln!("[ai-music-mcp] Request exceeds {MAX_REQUEST_SIZE} bytes, skipping");
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let resp = match req.method.as_str() {
            "initialize" => {
                json!({"jsonrpc":"2.0","id":req.id,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"ai-music","version":"0.1.0"}}})
            }
            "notifications/initialized" => continue,
            "tools/list" => {
                json!({"jsonrpc":"2.0","id":req.id,"result":{"tools":tool_definitions()}})
            }
            "tools/call" => {
                let params = req.params.unwrap_or(json!({}));
                let name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let result = call_tool(name, &args, &config);
                json!({"jsonrpc":"2.0","id":req.id,"result":result})
            }
            _ => {
                json!({"jsonrpc":"2.0","id":req.id,"error":{"code":-32601,"message":"Method not found"}})
            }
        };
        println!("{}", serde_json::to_string(&resp).unwrap());
    }
}
