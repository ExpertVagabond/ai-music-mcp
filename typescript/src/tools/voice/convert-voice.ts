import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { runPython } from "../../subprocess.js";
import { loadConfig } from "../../config.js";
import { existsSync } from "node:fs";
import { resolve, basename } from "node:path";

const schema = z.object({
  input_file: z.string().describe("Path to input audio file with vocals"),
  model_name: z
    .string()
    .describe("RVC model filename (e.g., 'matthew.pth')"),
  pitch: z
    .number()
    .int()
    .min(-24)
    .max(24)
    .default(0)
    .describe("Pitch shift in semitones (-24 to +24, default: 0)"),
  index_path: z
    .string()
    .optional()
    .describe("Path to RVC index file (.index) for better quality"),
  f0_method: z
    .enum(["harvest", "pm", "rmvpe"])
    .default("rmvpe")
    .describe("F0 extraction method (rmvpe=best, harvest=good, pm=fast)"),
  output_file: z
    .string()
    .optional()
    .describe("Custom output path (default: ~/Desktop/AI-Music/converted/)"),
});

export const convertVoice: McpAction = {
  tool: {
    name: "music_convert_voice",
    description:
      "Convert vocals to a target voice using RVC (Retrieval-based Voice Conversion). Requires a trained voice model (.pth) in the RVC weights directory.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { input_file, model_name, pitch, index_path, f0_method, output_file } =
      schema.parse(request.params.arguments);
    const config = loadConfig();

    if (!existsSync(input_file)) {
      return errorResult(`Input file not found: ${input_file}`);
    }

    const outputPath =
      output_file ||
      resolve(config.convertedDir, `converted-${basename(input_file)}`);

    const args = [
      config.rvcInferScript,
      "--model_name",
      model_name,
      "--input_path",
      input_file,
      "--opt_path",
      outputPath,
      "--f0up_key",
      String(pitch),
      "--f0method",
      f0_method,
    ];

    if (index_path) {
      args.push("--index_path", index_path);
    }

    try {
      const result = await runPython(config.rvcVenv, args, {
        cwd: config.rvcDir,
        timeoutMs: config.convertTimeout,
        env: {
          OMP_NUM_THREADS: "1",
        },
      });

      if (result.exitCode !== 0) {
        return errorResult(
          `RVC failed (exit ${result.exitCode}):\n${result.stderr || result.stdout}`
        );
      }

      return textResult({
        status: "success",
        output_file: outputPath,
        model: model_name,
        pitch_shift: pitch,
        f0_method,
        duration_ms: result.durationMs,
        raw_output: result.stdout,
      });
    } catch (e) {
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
