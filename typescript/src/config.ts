import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { homedir } from "node:os";

export interface MusicConfig {
  musicStudioDir: string;
  rvcDir: string;
  outputDir: string;
  stemsDir: string;
  convertedDir: string;
  musicgenVenv: string;
  rvcVenv: string;
  musicgenScript: string;
  stemsScript: string;
  rvcInferScript: string;
  rvcWeightsDir: string;
  rvcIndexDir: string;
  generateTimeout: number;
  stemsTimeout: number;
  convertTimeout: number;
}

function env(key: string, fallback: string): string {
  return process.env[key] || fallback;
}

export function loadConfig(): MusicConfig {
  const musicStudioDir = env(
    "MUSIC_STUDIO_DIR",
    "/Volumes/Virtual Server/projects/ai-music-studio"
  );
  const rvcDir = env(
    "RVC_DIR",
    "/Volumes/Virtual Server/projects/ai-music-rvc"
  );
  const outputDir = env(
    "MUSIC_OUTPUT_DIR",
    resolve(homedir(), "Desktop/AI-Music")
  );

  return {
    musicStudioDir,
    rvcDir,
    outputDir,
    stemsDir: resolve(outputDir, "stems"),
    convertedDir: resolve(outputDir, "converted"),
    musicgenVenv: resolve(musicStudioDir, ".venv"),
    rvcVenv: resolve(rvcDir, ".venv"),
    musicgenScript: resolve(musicStudioDir, "generate-music.py"),
    stemsScript: resolve(musicStudioDir, "separate-stems.py"),
    rvcInferScript: resolve(rvcDir, "tools/cmd/infer_cli.py"),
    rvcWeightsDir: resolve(rvcDir, "assets/weights"),
    rvcIndexDir: resolve(rvcDir, "logs"),
    generateTimeout: parseInt(env("MUSIC_GENERATE_TIMEOUT", "600000")),
    stemsTimeout: parseInt(env("MUSIC_STEMS_TIMEOUT", "600000")),
    convertTimeout: parseInt(env("MUSIC_CONVERT_TIMEOUT", "300000")),
  };
}

export function validateConfig(config: MusicConfig): void {
  const pythonBin = resolve(config.musicgenVenv, "bin/python");
  if (!existsSync(pythonBin)) {
    console.error(
      `[ai-music-mcp] Warning: MusicGen venv not found (check MUSIC_STUDIO_DIR)`
    );
  }
  const rvcPython = resolve(config.rvcVenv, "bin/python");
  if (!existsSync(rvcPython)) {
    console.error(
      `[ai-music-mcp] Warning: RVC venv not found (check RVC_DIR)`
    );
  }
}
