import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { runPython } from "../../subprocess.js";
import { loadConfig } from "../../config.js";
import { existsSync, mkdirSync } from "node:fs";
import { resolve, basename, join } from "node:path";

const schema = z.object({
  prompt: z.string().describe("Text description of the music to generate"),
  duration: z
    .number()
    .int()
    .min(1)
    .max(30)
    .default(15)
    .describe("Duration in seconds"),
  musicgen_model: z
    .enum(["small", "medium", "large"])
    .default("small")
    .describe("MusicGen model size"),
  separate_stems: z
    .boolean()
    .default(true)
    .describe("Separate the generated audio into stems"),
  demucs_model: z
    .enum(["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"])
    .default("htdemucs")
    .describe("Demucs model for stem separation"),
  voice_model: z
    .string()
    .optional()
    .describe(
      "RVC voice model to apply to vocals (e.g., 'matthew.pth'). Omit to skip voice conversion."
    ),
  pitch: z
    .number()
    .int()
    .min(-24)
    .max(24)
    .default(0)
    .describe("Pitch shift for voice conversion"),
});

interface PipelineStep {
  step: string;
  status: "success" | "error" | "skipped";
  duration_ms?: number;
  file?: string;
  stem_dir?: string;
  stems?: string[];
  error?: string;
}

export const fullPipeline: McpAction = {
  tool: {
    name: "music_full_pipeline",
    description:
      "End-to-end music pipeline: generate instrumental with MusicGen, separate stems with Demucs, and optionally convert vocals with RVC. Can take 5-15 minutes depending on settings.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();
    const steps: PipelineStep[] = [];
    const totalStart = Date.now();

    // Step 1: Generate music
    const genArgs = [
      config.musicgenScript,
      args.prompt,
      "--duration",
      String(args.duration),
      "--model",
      args.musicgen_model,
    ];

    try {
      const genResult = await runPython(config.musicgenVenv, genArgs, {
        cwd: config.musicStudioDir,
        timeoutMs: config.generateTimeout,
      });

      if (genResult.exitCode !== 0) {
        steps.push({
          step: "generate",
          status: "error",
          duration_ms: genResult.durationMs,
          error: genResult.stderr || genResult.stdout,
        });
        return textResult({
          pipeline_steps: steps,
          total_time_ms: Date.now() - totalStart,
        });
      }

      const savedMatch = genResult.stdout.match(/Saved:\s*(.+)/);
      const generatedFile = savedMatch?.[1]?.trim() ?? "";

      steps.push({
        step: "generate",
        status: "success",
        duration_ms: genResult.durationMs,
        file: generatedFile,
      });

      // Step 2: Separate stems
      if (!args.separate_stems || !generatedFile || !existsSync(generatedFile)) {
        steps.push({ step: "separate", status: "skipped" });
      } else {
        const stemArgs = [
          config.stemsScript,
          generatedFile,
          "--model",
          args.demucs_model,
        ];

        const stemResult = await runPython(config.musicgenVenv, stemArgs, {
          cwd: config.musicStudioDir,
          timeoutMs: config.stemsTimeout,
        });

        if (stemResult.exitCode !== 0) {
          steps.push({
            step: "separate",
            status: "error",
            duration_ms: stemResult.durationMs,
            error: stemResult.stderr || stemResult.stdout,
          });
        } else {
          const stemDirMatch = stemResult.stdout.match(
            /Stems saved to:\s*(.+)/
          );
          const stemDir = stemDirMatch?.[1]?.trim() ?? "";
          const stemFiles: string[] = [];
          const stemRegex = /^\s+(\S+\.wav)\s+\(/gm;
          let m;
          while ((m = stemRegex.exec(stemResult.stdout)) !== null) {
            stemFiles.push(m[1]);
          }

          steps.push({
            step: "separate",
            status: "success",
            duration_ms: stemResult.durationMs,
            stem_dir: stemDir,
            stems: stemFiles,
          });

          // Step 3: Voice conversion on vocals
          if (!args.voice_model) {
            steps.push({ step: "voice_convert", status: "skipped" });
          } else {
            const vocalsPath = join(stemDir, "vocals.wav");
            if (!existsSync(vocalsPath)) {
              steps.push({
                step: "voice_convert",
                status: "error",
                error: `vocals.wav not found at ${vocalsPath}`,
              });
            } else {
              mkdirSync(config.convertedDir, { recursive: true });
              const outputPath = resolve(
                config.convertedDir,
                `converted-vocals-${basename(generatedFile)}`
              );

              const vcArgs = [
                config.rvcInferScript,
                "--model_name",
                args.voice_model,
                "--input_path",
                vocalsPath,
                "--opt_path",
                outputPath,
                "--f0up_key",
                String(args.pitch),
                "--f0method",
                "rmvpe",
              ];

              const vcResult = await runPython(config.rvcVenv, vcArgs, {
                cwd: config.rvcDir,
                timeoutMs: config.convertTimeout,
                env: { OMP_NUM_THREADS: "1" },
              });

              if (vcResult.exitCode !== 0) {
                steps.push({
                  step: "voice_convert",
                  status: "error",
                  duration_ms: vcResult.durationMs,
                  error: vcResult.stderr || vcResult.stdout,
                });
              } else {
                steps.push({
                  step: "voice_convert",
                  status: "success",
                  duration_ms: vcResult.durationMs,
                  file: outputPath,
                });
              }
            }
          }
        }
      }

      return textResult({
        pipeline_steps: steps,
        total_time_ms: Date.now() - totalStart,
      });
    } catch (e) {
      steps.push({
        step: "unknown",
        status: "error",
        error: e instanceof Error ? e.message : String(e),
      });
      return textResult({
        pipeline_steps: steps,
        total_time_ms: Date.now() - totalStart,
      });
    }
  },
};
