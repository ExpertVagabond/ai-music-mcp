import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { runPython } from "../../subprocess.js";
import { loadConfig } from "../../config.js";
import { existsSync } from "node:fs";

const schema = z.object({
  input_file: z
    .string()
    .describe("Path to input audio file (MP3, WAV, FLAC)"),
  model: z
    .enum(["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"])
    .default("htdemucs")
    .describe("Demucs model (htdemucs=fast, htdemucs_ft=best quality)"),
  two_stems: z
    .enum(["vocals", "drums", "bass", "other"])
    .optional()
    .describe(
      "Separate into only two stems (e.g., vocals + everything else)"
    ),
  output_dir: z
    .string()
    .optional()
    .describe("Custom output directory (default: ~/Desktop/AI-Music/stems/)"),
});

export const separateStems: McpAction = {
  tool: {
    name: "music_separate_stems",
    description:
      "Separate audio into stems (vocals, drums, bass, other) using Demucs. Input any audio file, get individual stem WAV files.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { input_file, model, two_stems, output_dir } = schema.parse(
      request.params.arguments
    );
    const config = loadConfig();

    if (!existsSync(input_file)) {
      return errorResult(`Input file not found: ${input_file}`);
    }

    const args = [config.stemsScript, input_file, "--model", model];
    if (output_dir) {
      args.push("--output", output_dir);
    }
    if (two_stems) {
      args.push("--two-stems", two_stems);
    }

    try {
      const result = await runPython(config.musicgenVenv, args, {
        cwd: config.musicStudioDir,
        timeoutMs: config.stemsTimeout,
      });

      if (result.exitCode !== 0) {
        return errorResult(
          `Demucs failed (exit ${result.exitCode}):\n${result.stderr || result.stdout}`
        );
      }

      // Parse output
      const stemDirMatch = result.stdout.match(/Stems saved to:\s*(.+)/);
      const stemFiles: string[] = [];
      const stemRegex = /^\s+(\S+\.wav)\s+\(/gm;
      let match;
      while ((match = stemRegex.exec(result.stdout)) !== null) {
        stemFiles.push(match[1]);
      }

      return textResult({
        status: "success",
        stem_dir: stemDirMatch?.[1] ?? "unknown",
        stems: stemFiles,
        model,
        raw_output: result.stdout,
      });
    } catch (e) {
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
