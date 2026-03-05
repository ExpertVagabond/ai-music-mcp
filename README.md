# ai-music-mcp

**MCP server for local AI music production.** Generate instrumentals with MusicGen, separate stems with Demucs, convert voices with RVC -- orchestrated through Claude Code or any MCP-compatible client.

[![npm](https://img.shields.io/npm/v/ai-music-mcp)](https://www.npmjs.com/package/ai-music-mcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.3-blue?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![MCP SDK](https://img.shields.io/badge/MCP_SDK-1.8-purple)](https://modelcontextprotocol.io)

---

## What It Does

This server exposes 8 tools over the [Model Context Protocol](https://modelcontextprotocol.io) that give AI assistants full control of a local music production pipeline. No cloud APIs, no rate limits -- everything runs on your machine using open-source models.

```
  Prompt                                                     Final Output
    |                                                             |
    v                                                             v
+----------+     +----------+     +-----------+     +-----------+
| MusicGen | --> | Demucs   | --> | RVC Voice | --> | Mixed     |
| Generate |     | Separate |     | Convert   |     | Audio     |
+----------+     +----------+     +-----------+     +-----------+
  "dark trap       vocals.wav      converted-        Ready to
   beat with       drums.wav       vocals.wav         use
   808s"           bass.wav
                   other.wav
```

The `music_full_pipeline` tool chains all three stages in a single call -- generate an instrumental from a text prompt, split it into stems, and apply a trained voice model to the vocals.

## Tools

| Tool | Description |
|---|---|
| `music_generate` | Generate instrumental music from a text prompt using MusicGen (small/medium/large models, 1-30s duration) |
| `music_separate_stems` | Separate any audio file into vocals, drums, bass, and other stems using Demucs |
| `music_list_stems` | Browse separated stems by track name and Demucs model |
| `music_convert_voice` | Convert vocals to a target voice using RVC with pitch shifting and F0 method selection |
| `music_list_voice_models` | List trained RVC voice models (.pth) and their associated index files |
| `music_list_files` | List all generated audio files by category (generated, stems, converted) |
| `music_get_info` | Get audio file metadata -- duration, sample rate, channels, codec, bitrate (via ffprobe) |
| `music_full_pipeline` | End-to-end pipeline: generate + separate stems + voice convert in one call (5-15 min) |

## Prerequisites

| Dependency | Purpose | Install |
|---|---|---|
| **Node.js 18+** | MCP server runtime | [nodejs.org](https://nodejs.org) |
| **Python 3.11+** | ML model execution | `brew install python@3.11` |
| **PyTorch** | Neural network backend | [pytorch.org](https://pytorch.org) |
| **ffmpeg / ffprobe** | Audio processing and metadata | `brew install ffmpeg` |
| **[ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio)** | MusicGen + Demucs Python scripts | Clone and set up venv |
| **[RVC WebUI](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI)** | Voice conversion (optional) | Clone and set up venv |

## Install

### From npm

```bash
npm install -g ai-music-mcp
```

### From source

```bash
git clone https://github.com/ExpertVagabond/ai-music-mcp.git
cd ai-music-mcp/typescript
npm install
npm run build
```

## Configuration

### Claude Desktop

Add to `~/.claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ai-music": {
      "command": "ai-music-mcp",
      "env": {
        "MUSIC_STUDIO_DIR": "/path/to/ai-music-studio",
        "RVC_DIR": "/path/to/ai-music-rvc",
        "MUSIC_OUTPUT_DIR": "~/Desktop/AI-Music"
      }
    }
  }
}
```

### Claude Code

Add to `~/.mcp.json`:

```json
{
  "mcpServers": {
    "ai-music": {
      "command": "ai-music-mcp",
      "env": {
        "MUSIC_STUDIO_DIR": "/path/to/ai-music-studio",
        "RVC_DIR": "/path/to/ai-music-rvc",
        "MUSIC_OUTPUT_DIR": "~/Desktop/AI-Music"
      }
    }
  }
}
```

### Running from source

If running from a local clone instead of the npm package:

```json
{
  "mcpServers": {
    "ai-music": {
      "command": "node",
      "args": ["/path/to/ai-music-mcp/typescript/build/index.js"],
      "env": {
        "MUSIC_STUDIO_DIR": "/path/to/ai-music-studio",
        "RVC_DIR": "/path/to/ai-music-rvc",
        "MUSIC_OUTPUT_DIR": "~/Desktop/AI-Music"
      }
    }
  }
}
```

### Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `MUSIC_STUDIO_DIR` | Yes | -- | Path to [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) with MusicGen/Demucs scripts and Python venv |
| `RVC_DIR` | No | -- | Path to RVC WebUI installation with its own Python venv (needed for voice conversion) |
| `MUSIC_OUTPUT_DIR` | No | `~/Desktop/AI-Music` | Output directory for all generated files |
| `MUSIC_GENERATE_TIMEOUT` | No | `600000` | MusicGen timeout in milliseconds (10 min) |
| `MUSIC_STEMS_TIMEOUT` | No | `600000` | Demucs timeout in milliseconds (10 min) |
| `MUSIC_CONVERT_TIMEOUT` | No | `300000` | RVC timeout in milliseconds (5 min) |

## Usage Examples

### Generate an instrumental

```
> Generate a 20-second dark trap beat with heavy 808s and hi-hats

Calls: music_generate
  prompt: "dark trap beat with heavy 808s and hi-hats"
  duration: 20
  model: "small"
```

### Separate stems from any audio file

```
> Separate the stems from ~/Music/song.mp3 using the fine-tuned model

Calls: music_separate_stems
  input_file: "~/Music/song.mp3"
  model: "htdemucs_ft"
```

### Convert vocals to a different voice

```
> Convert the vocals to matthew's voice, shift pitch up 2 semitones

Calls: music_convert_voice
  input_file: "~/Desktop/AI-Music/stems/htdemucs/song/vocals.wav"
  model_name: "matthew.pth"
  pitch: 2
  f0_method: "rmvpe"
```

### Full pipeline in one call

```
> Create a lo-fi chill beat, separate it, and apply my voice model

Calls: music_full_pipeline
  prompt: "lo-fi chill beat with warm piano and vinyl crackle"
  duration: 15
  musicgen_model: "small"
  separate_stems: true
  demucs_model: "htdemucs"
  voice_model: "matthew.pth"
```

## Output Structure

```
~/Desktop/AI-Music/
├── dark-trap-beat-808s.wav           # Generated instrumentals
├── lofi-chill-beat-piano.wav
├── stems/
│   └── htdemucs/
│       └── dark-trap-beat-808s/
│           ├── vocals.wav            # Isolated vocals
│           ├── drums.wav             # Isolated drums
│           ├── bass.wav              # Isolated bass
│           └── other.wav             # Everything else
└── converted/
    └── converted-vocals-dark-trap.wav  # RVC voice conversion output
```

## Architecture

```
ai-music-mcp/
├── typescript/
│   ├── src/
│   │   ├── index.ts              # MCP server entry point
│   │   ├── config.ts             # Environment and path configuration
│   │   ├── subprocess.ts         # Python venv subprocess runner
│   │   ├── types.ts              # McpAction type, result helpers
│   │   └── tools/
│   │       ├── generate/
│   │       │   └── generate-music.ts
│   │       ├── stems/
│   │       │   ├── separate-stems.ts
│   │       │   └── list-stems.ts
│   │       ├── voice/
│   │       │   ├── convert-voice.ts
│   │       │   └── list-voice-models.ts
│   │       ├── files/
│   │       │   ├── list-files.ts
│   │       │   └── get-info.ts
│   │       └── pipeline/
│   │           └── full-pipeline.ts
│   ├── esbuild.config.js         # Bundles to single executable
│   ├── tsconfig.json
│   └── package.json
├── ai-music-mcp-wrapper.sh       # Local development launcher
└── README.md
```

The server uses the [MCP SDK](https://github.com/modelcontextprotocol/typescript-sdk) with stdio transport. Each tool is defined as a `McpAction` with a Zod schema for input validation and a handler that spawns Python subprocesses inside isolated virtual environments. The build step compiles TypeScript then bundles everything with esbuild into a single `#!/usr/bin/env node` executable.

## Development

```bash
cd typescript
npm install
npm run build        # Compile + bundle
npm run watch        # TypeScript watch mode
npm run inspector    # Launch MCP Inspector for interactive testing
```

## Related Projects

- [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) -- Python CLI for MusicGen and Demucs (used as the backend)
- [rvc-mcp](https://github.com/ExpertVagabond/rvc-mcp) -- MCP server for RVC model training and management
- [music-distro](https://github.com/ExpertVagabond/music-distro) -- MCP server for distributing tracks to SoundCloud and YouTube

## Contributing

Contributions are welcome. Please open an issue first to discuss what you would like to change.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m "Add my feature"`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a Pull Request

## License

[MIT](LICENSE) -- Purple Squirrel Media
