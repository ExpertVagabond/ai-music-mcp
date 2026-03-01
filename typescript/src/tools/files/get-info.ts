import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig } from "../../config.js";
import { existsSync, statSync } from "node:fs";
import { execSync } from "node:child_process";
import { basename, extname } from "node:path";

const schema = z.object({
  file_path: z.string().describe("Path to audio file"),
});

export const getInfo: McpAction = {
  tool: {
    name: "music_get_info",
    description:
      "Get audio file metadata: duration, sample rate, channels, format, and file size.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { file_path } = schema.parse(request.params.arguments);

    if (!existsSync(file_path)) {
      return errorResult(`File not found: ${file_path}`);
    }

    const stats = statSync(file_path);
    const ext = extname(file_path).toLowerCase();

    const info: Record<string, unknown> = {
      file: basename(file_path),
      path: file_path,
      size_mb: parseFloat((stats.size / 1024 / 1024).toFixed(2)),
      format: ext.replace(".", ""),
      modified: stats.mtime.toISOString(),
    };

    // Try ffprobe for detailed metadata
    try {
      const probeOutput = execSync(
        `ffprobe -v quiet -print_format json -show_format -show_streams "${file_path}"`,
        { timeout: 10000 }
      ).toString();

      const probe = JSON.parse(probeOutput);
      const audioStream = probe.streams?.find(
        (s: Record<string, string>) => s.codec_type === "audio"
      );

      if (audioStream) {
        info.sample_rate = parseInt(audioStream.sample_rate);
        info.channels = parseInt(audioStream.channels);
        info.codec = audioStream.codec_name;
        info.bit_depth = audioStream.bits_per_sample
          ? parseInt(audioStream.bits_per_sample)
          : null;
      }
      if (probe.format) {
        info.duration_seconds = parseFloat(
          parseFloat(probe.format.duration).toFixed(2)
        );
        info.bitrate_kbps = probe.format.bit_rate
          ? Math.round(parseInt(probe.format.bit_rate) / 1000)
          : null;
      }
    } catch {
      // ffprobe not available, return basic info
      info.note = "Install ffprobe for detailed audio metadata";
    }

    return textResult(info);
  },
};
