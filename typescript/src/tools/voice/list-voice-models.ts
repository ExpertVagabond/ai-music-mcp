import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult } from "../../types.js";
import { loadConfig } from "../../config.js";
import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const schema = z.object({});

interface VoiceModel {
  name: string;
  size_mb: number;
  has_index: boolean;
  index_path: string | null;
}

export const listVoiceModels: McpAction = {
  tool: {
    name: "music_list_voice_models",
    description:
      "List trained RVC voice models (.pth files) available for voice conversion.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async () => {
    const config = loadConfig();
    const models: VoiceModel[] = [];

    if (!existsSync(config.rvcWeightsDir)) {
      return textResult({
        models: [],
        weights_dir: config.rvcWeightsDir,
        message: "No weights directory found. Train a voice model first.",
      });
    }

    const files = readdirSync(config.rvcWeightsDir).filter((f) =>
      f.endsWith(".pth")
    );

    for (const f of files) {
      const fullPath = join(config.rvcWeightsDir, f);
      const stats = statSync(fullPath);
      const modelName = f.replace(".pth", "");

      // Check for matching index file in logs/
      let indexPath: string | null = null;
      if (existsSync(config.rvcIndexDir)) {
        const indexDir = join(config.rvcIndexDir, modelName);
        if (existsSync(indexDir)) {
          const indexFiles = readdirSync(indexDir).filter((x) =>
            x.endsWith(".index")
          );
          if (indexFiles.length > 0) {
            indexPath = join(indexDir, indexFiles[0]);
          }
        }
      }

      models.push({
        name: f,
        size_mb: parseFloat((stats.size / 1024 / 1024).toFixed(1)),
        has_index: indexPath !== null,
        index_path: indexPath,
      });
    }

    return textResult({
      models,
      total: models.length,
      weights_dir: config.rvcWeightsDir,
    });
  },
};
