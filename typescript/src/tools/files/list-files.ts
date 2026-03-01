import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult } from "../../types.js";
import { loadConfig } from "../../config.js";
import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const schema = z.object({
  directory: z
    .enum(["all", "generated", "stems", "converted"])
    .default("all")
    .describe("Which directory to list (default: all)"),
  pattern: z
    .string()
    .optional()
    .describe("Filter files by extension (e.g., '.wav', '.mp3')"),
});

interface FileInfo {
  name: string;
  path: string;
  size_mb: number;
  modified: string;
  category: string;
}

function listDir(dir: string, category: string, pattern?: string): FileInfo[] {
  if (!existsSync(dir)) return [];

  const files: FileInfo[] = [];
  const entries = readdirSync(dir);

  for (const entry of entries) {
    const fullPath = join(dir, entry);
    const stats = statSync(fullPath);

    if (stats.isFile()) {
      if (pattern && !entry.endsWith(pattern)) continue;
      files.push({
        name: entry,
        path: fullPath,
        size_mb: parseFloat((stats.size / 1024 / 1024).toFixed(2)),
        modified: stats.mtime.toISOString(),
        category,
      });
    }
  }

  return files;
}

export const listFiles: McpAction = {
  tool: {
    name: "music_list_files",
    description:
      "List generated audio files in the AI Music output directory. Filter by category (generated, stems, converted) and file extension.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { directory, pattern } = schema.parse(request.params.arguments);
    const config = loadConfig();

    let files: FileInfo[] = [];

    if (directory === "all" || directory === "generated") {
      files.push(...listDir(config.outputDir, "generated", pattern));
    }
    if (directory === "all" || directory === "converted") {
      files.push(...listDir(config.convertedDir, "converted", pattern));
    }
    if (directory === "all" || directory === "stems") {
      // Stems are nested: stems/{model}/{track}/*.wav
      if (existsSync(config.stemsDir)) {
        for (const model of readdirSync(config.stemsDir)) {
          const modelDir = join(config.stemsDir, model);
          if (!statSync(modelDir).isDirectory()) continue;
          for (const track of readdirSync(modelDir)) {
            const trackDir = join(modelDir, track);
            if (!statSync(trackDir).isDirectory()) continue;
            files.push(...listDir(trackDir, `stems/${model}/${track}`, pattern));
          }
        }
      }
    }

    files.sort((a, b) => b.modified.localeCompare(a.modified));

    return textResult({
      files,
      total: files.length,
      output_dir: config.outputDir,
    });
  },
};
