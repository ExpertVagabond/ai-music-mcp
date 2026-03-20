import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

import type { McpAction } from "./types.js";
import { errorResult } from "./types.js";
import { loadConfig, validateConfig } from "./config.js";

// Tools
import { generateMusic } from "./tools/generate/generate-music.js";
import { separateStems } from "./tools/stems/separate-stems.js";
import { listStems } from "./tools/stems/list-stems.js";
import { convertVoice } from "./tools/voice/convert-voice.js";
import { listVoiceModels } from "./tools/voice/list-voice-models.js";
import { listFiles } from "./tools/files/list-files.js";
import { getInfo } from "./tools/files/get-info.js";
import { fullPipeline } from "./tools/pipeline/full-pipeline.js";

const actions: McpAction[] = [
  generateMusic,
  separateStems,
  listStems,
  convertVoice,
  listVoiceModels,
  listFiles,
  getInfo,
  fullPipeline,
];

const server = new Server(
  {
    name: "ai-music",
    version: "0.1.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: actions.map((a) => a.tool),
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const action = actions.find((a) => a.tool.name === request.params.name);
  if (!action) {
    return errorResult(`Unknown tool: ${request.params.name}`);
  }
  return action.handler(request);
});

// Validate config on startup
const config = loadConfig();
validateConfig(config);
console.error(`[ai-music-mcp] Starting with ${actions.length} tools`);
console.error(`[ai-music-mcp] MusicGen: [configured]`);
console.error(`[ai-music-mcp] RVC: [configured]`);
console.error(`[ai-music-mcp] Output: [configured]`);

const transport = new StdioServerTransport();
await server.connect(transport);
