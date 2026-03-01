import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig } from "../../config.js";
import { existsSync, readdirSync, statSync } from "node:fs";
import { resolve, join } from "node:path";

const schema = z.object({
  track_name: z
    .string()
    .optional()
    .describe("Filter by track name (default: list all)"),
  model: z
    .enum(["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"])
    .optional()
    .describe("Filter by Demucs model used"),
});

interface StemInfo {
  name: string;
  size_mb: number;
}

interface TrackInfo {
  name: string;
  model: string;
  path: string;
  stems: StemInfo[];
}

export const listStems: McpAction = {
  tool: {
    name: "music_list_stems",
    description:
      "List available separated stems. Shows all tracks that have been separated with their individual stem files.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { track_name, model } = schema.parse(request.params.arguments);
    const config = loadConfig();

    if (!existsSync(config.stemsDir)) {
      return textResult({ tracks: [], message: "No stems directory found" });
    }

    const tracks: TrackInfo[] = [];
    const models = model
      ? [model]
      : readdirSync(config.stemsDir).filter((f) => {
          const p = join(config.stemsDir, f);
          return statSync(p).isDirectory();
        });

    for (const m of models) {
      const modelDir = join(config.stemsDir, m);
      if (!existsSync(modelDir)) continue;

      const trackDirs = readdirSync(modelDir).filter((f) => {
        const p = join(modelDir, f);
        return statSync(p).isDirectory();
      });

      for (const t of trackDirs) {
        if (track_name && !t.includes(track_name)) continue;

        const trackPath = join(modelDir, t);
        const stemFiles = readdirSync(trackPath)
          .filter((f) => f.endsWith(".wav"))
          .map((f) => ({
            name: f,
            size_mb: parseFloat(
              (statSync(join(trackPath, f)).size / 1024 / 1024).toFixed(1)
            ),
          }));

        tracks.push({
          name: t,
          model: m,
          path: trackPath,
          stems: stemFiles,
        });
      }
    }

    return textResult({ tracks, total: tracks.length });
  },
};
