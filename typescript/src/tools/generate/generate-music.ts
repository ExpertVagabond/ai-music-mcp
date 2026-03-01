import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { runPython } from "../../subprocess.js";
import { loadConfig } from "../../config.js";

const schema = z.object({
  prompt: z.string().describe("Text description of the music to generate"),
  duration: z
    .number()
    .int()
    .min(1)
    .max(30)
    .default(15)
    .describe("Duration in seconds (1-30, default: 15)"),
  model: z
    .enum(["small", "medium", "large"])
    .default("small")
    .describe("MusicGen model size (small=fast, large=best quality)"),
  output_filename: z
    .string()
    .optional()
    .describe("Custom output filename (default: auto-generated from prompt)"),
});

export const generateMusic: McpAction = {
  tool: {
    name: "music_generate",
    description:
      "Generate instrumental music from a text prompt using MusicGen. Returns a WAV file. Takes ~15x real-time on CPU (e.g., 15s clip takes ~4 minutes).",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { prompt, duration, model, output_filename } = schema.parse(
      request.params.arguments
    );
    const config = loadConfig();

    const args = [config.musicgenScript, prompt, "--duration", String(duration), "--model", model];
    if (output_filename) {
      args.push("--output", output_filename);
    }

    try {
      const result = await runPython(config.musicgenVenv, args, {
        cwd: config.musicStudioDir,
        timeoutMs: config.generateTimeout,
      });

      if (result.exitCode !== 0) {
        return errorResult(
          `MusicGen failed (exit ${result.exitCode}):\n${result.stderr || result.stdout}`
        );
      }

      // Parse output for file path and timing
      const savedMatch = result.stdout.match(/Saved:\s*(.+)/);
      const timeMatch = result.stdout.match(/Generated in ([\d.]+)s/);
      const rateMatch = result.stdout.match(/\(([\d.]+)x real-time\)/);
      const srMatch = result.stdout.match(/Sample rate:\s*(\d+)/);

      return textResult({
        status: "success",
        file: savedMatch?.[1] ?? "unknown",
        duration_seconds: duration,
        model,
        sample_rate: srMatch ? parseInt(srMatch[1]) : 32000,
        generation_time_seconds: timeMatch ? parseFloat(timeMatch[1]) : null,
        realtime_factor: rateMatch ? parseFloat(rateMatch[1]) : null,
        raw_output: result.stdout,
      });
    } catch (e) {
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
