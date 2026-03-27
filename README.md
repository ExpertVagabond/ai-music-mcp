# ai-music-mcp

**The only local AI music production MCP -- generate, separate stems, convert voice, full pipeline. No API costs.**

[![npm](https://img.shields.io/npm/v/ai-music-mcp)](https://www.npmjs.com/package/ai-music-mcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tools: 8](https://img.shields.io/badge/tools-8-green)]()

---

MusicGen instrumental generation, Demucs stem separation, and RVC voice conversion -- orchestrated through a single MCP server that runs entirely on your machine. No cloud APIs, no rate limits, no per-track charges. Feed it a text prompt, get back a produced track with isolated stems and converted vocals.

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

## Install

```bash
npm install -g ai-music-mcp
```

## Configure

Add to `claude_desktop_config.json` or `~/.mcp.json`:

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

### Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `MUSIC_STUDIO_DIR` | Yes | -- | Path to [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) with MusicGen/Demucs scripts and Python venv |
| `RVC_DIR` | No | -- | Path to RVC WebUI installation (needed for voice conversion) |
| `MUSIC_OUTPUT_DIR` | No | `~/Desktop/AI-Music` | Output directory for all generated files |
| `MUSIC_GENERATE_TIMEOUT` | No | `600000` | MusicGen timeout in ms (10 min) |
| `MUSIC_STEMS_TIMEOUT` | No | `600000` | Demucs timeout in ms (10 min) |
| `MUSIC_CONVERT_TIMEOUT` | No | `300000` | RVC timeout in ms (5 min) |

## Tool Reference

| Tool | Description |
|---|---|
| `music_generate` | Generate instrumental music from a text prompt via MusicGen (small/medium/large, 1--30s) |
| `music_separate_stems` | Separate any audio into vocals, drums, bass, and other stems via Demucs |
| `music_convert_voice` | Convert vocals to a target voice with RVC -- pitch shifting, F0 method selection |
| `music_list_voice_models` | List trained RVC voice models (.pth) and their index files |
| `music_full_pipeline` | End-to-end: generate + separate stems + voice convert in one call (5--15 min) |
| `music_list_files` | List all generated audio by category (generated, stems, converted) |
| `music_list_stems` | Browse separated stems by track name and Demucs model |
| `music_get_info` | Audio file metadata -- duration, sample rate, channels, codec, bitrate (ffprobe) |

## Usage Examples

```
> Generate a 20-second dark trap beat with heavy 808s and hi-hats
  -> music_generate { prompt: "dark trap beat...", duration: 20, model: "small" }

> Separate the stems from ~/Music/song.mp3
  -> music_separate_stems { input_file: "~/Music/song.mp3", model: "htdemucs_ft" }

> Convert the vocals to matthew's voice, shift pitch up 2 semitones
  -> music_convert_voice { input_file: ".../vocals.wav", model_name: "matthew.pth", pitch: 2 }

> Full pipeline: lo-fi chill beat, separate it, apply my voice model
  -> music_full_pipeline { prompt: "lo-fi chill beat with warm piano", voice_model: "matthew.pth" }
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

## Prerequisites

| Dependency | Purpose | Install |
|---|---|---|
| **Node.js 18+** | MCP server runtime | [nodejs.org](https://nodejs.org) |
| **Python 3.11+** | ML model execution | `brew install python@3.11` |
| **PyTorch** | Neural network backend | [pytorch.org](https://pytorch.org) |
| **ffmpeg / ffprobe** | Audio processing and metadata | `brew install ffmpeg` |
| **[ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio)** | MusicGen + Demucs Python scripts | Clone and set up venv |
| **[RVC WebUI](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI)** | Voice conversion (optional) | Clone and set up venv |

## Why This One?

| | ai-music-mcp | [musicmcp.ai](https://musicmcp.ai) | MiniMax Music API |
|---|---|---|---|
| **Runs locally** | Yes -- zero network calls | No -- cloud only | No -- cloud only |
| **Per-track cost** | $0 | Paid per generation | Paid per API call |
| **Stem separation** | Built-in (Demucs) | No | No |
| **Voice conversion** | Built-in (RVC) | No | No |
| **Full pipeline** | Generate + stems + voice in one call | Generate only | Generate only |
| **Model control** | Choose MusicGen small/medium/large, Demucs model, RVC model | Fixed | Fixed |

The only MCP server that chains generation, stem separation, and voice conversion into a single local pipeline. Every other option is cloud-only and charges per track.

## Development

```bash
cd typescript
npm install
npm run build        # Compile + bundle
npm run watch        # TypeScript watch mode
npm run inspector    # Launch MCP Inspector for interactive testing
```

## Related Projects

- [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) -- Python CLI for MusicGen and Demucs (backend)
- [rvc-mcp](https://github.com/ExpertVagabond/rvc-mcp) -- MCP server for RVC model training and management
- [music-distro](https://github.com/ExpertVagabond/music-distro) -- MCP server for distributing tracks to SoundCloud and YouTube

## License

[MIT](LICENSE) -- Purple Squirrel Media
