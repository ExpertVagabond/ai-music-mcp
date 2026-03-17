use serde::Deserialize;
use serde_json::{Value, json};
use std::io::BufRead;
use std::process::Command;

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

impl Config {
    fn from_env() -> Self {
        let music_studio_dir = std::env::var("MUSIC_STUDIO_DIR")
            .unwrap_or_else(|_| "/Volumes/Virtual Server/projects/ai-music-studio".into());
        let rvc_dir = std::env::var("RVC_DIR")
            .unwrap_or_else(|_| "/Volumes/Virtual Server/projects/ai-music-rvc".into());
        let home = std::env::var("HOME").unwrap_or_default();
        let output_dir = std::env::var("MUSIC_OUTPUT_DIR")
            .unwrap_or_else(|_| format!("{}/Desktop/AI-Music", home));
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
    let python = format!("{}/bin/python", venv);
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
                    "voice_model": { "type": "string", "description": "RVC voice model for conversion (optional)" },
                    "pitch": { "type": "integer", "description": "Pitch shift for voice conversion", "default": 0 }
                },
                "required": ["prompt"]
            }
        }
    ])
}

fn call_tool(name: &str, args: &Value, config: &Config) -> Value {
    let text = match name {
        "music_generate" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            let duration = args["duration"].as_i64().unwrap_or(15).to_string();
            let model = args["model"].as_str().unwrap_or("small");
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
            let model = args["model"].as_str().unwrap_or("htdemucs");
            let stems = args["stems"].as_str().unwrap_or("all");
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
            let model_name = args["model_name"].as_str().unwrap_or("");
            let pitch = args["pitch"].as_i64().unwrap_or(0).to_string();
            let index_rate = args["index_rate"].as_f64().unwrap_or(0.75).to_string();
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
                        if n.ends_with(".pth") { Some(n) } else { None }
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
            let sub = args["subdirectory"].as_str().unwrap_or("");
            let dir = match sub {
                "stems" => &config.stems_dir,
                "converted" => &config.converted_dir,
                _ => &config.output_dir,
            };
            list_dir(dir)
        }
        "music_get_info" => {
            let path = args["file_path"].as_str().unwrap_or("");
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
            let duration = args["duration"].as_i64().unwrap_or(15).to_string();
            let model = args["model"].as_str().unwrap_or("small");
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
                    // Try to extract file path from output
                    if let Some(voice_model) = args["voice_model"].as_str()
                        && let Some(cap) = gen_out.lines().find(|l| l.contains("Saved:"))
                    {
                        let saved_path = cap.replace("Saved:", "").trim().to_string();
                        let pitch = args["pitch"].as_i64().unwrap_or(0).to_string();
                        let mp = format!("{}/{}", config.rvc_weights_dir, voice_model);
                        let conv_args = vec![
                            config.rvc_infer_script.as_str(),
                            &saved_path,
                            "--model",
                            &mp,
                            "--pitch",
                            &pitch,
                            "--output-dir",
                            &config.converted_dir,
                        ];
                        match run_python(&config.rvc_venv, &conv_args, &config.rvc_dir, 300) {
                            Ok(conv_out) => {
                                result["step2_voice_convert"] = json!("success");
                                result["convert_output"] = json!(conv_out);
                            }
                            Err(e) => {
                                result["step2_voice_convert"] = json!(format!("error: {e}"));
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
        "[ai-music-mcp] MusicGen: {}, RVC: {}, Output: {}",
        config.music_studio_dir, config.rvc_dir, config.output_dir
    );
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
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
