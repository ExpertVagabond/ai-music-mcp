# ai-music-mcp

MCP server for local AI music production. Generate instrumentals with MusicGen, separate stems with Demucs, convert voices with RVC — all through Claude Code or any MCP client.

## Tools (8)

| Tool | Description |
|---|---|
| `music_generate` | Generate instrumental music from a text prompt (MusicGen small/medium/large) |
| `music_separate_stems` | Separate audio into vocals, drums, bass, other (Demucs) |
| `music_list_stems` | List available separated stems by track/model |
| `music_convert_voice` | Convert vocals to a target voice (RVC, pitch shift, F0 methods) |
| `music_list_voice_models` | List trained RVC voice models (.pth) and index files |
| `music_list_files` | List generated audio files by category (generated/stems/converted) |
| `music_get_info` | Get audio file metadata (duration, sample rate, codec, bitrate) |
| `music_full_pipeline` | End-to-end: generate → separate stems → voice convert (5-15 min) |

## Quick Start

### Prerequisites

- Node.js 18+
- Python 3.11+ with PyTorch, Transformers, Demucs
- ffmpeg (`brew install ffmpeg`)
- [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) (MusicGen + Demucs scripts)
- [RVC WebUI](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI) (optional, for voice conversion)

### Install

```bash
npm install -g ai-music-mcp
```

### Configure in Claude Code

Add to your MCP config (`~/.mcp.json`):

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

### Or run from source

```bash
git clone https://github.com/ExpertVagabond/ai-music-mcp.git
cd ai-music-mcp/typescript
npm install && npm run build
node build/index.js
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `MUSIC_STUDIO_DIR` | — | Path to ai-music-studio (MusicGen/Demucs scripts) |
| `RVC_DIR` | — | Path to RVC WebUI installation |
| `MUSIC_OUTPUT_DIR` | `~/Desktop/AI-Music` | Output directory for all generated files |

## Tool Details

### music_generate

```
"dark trap beat with heavy 808s and hi-hats"
--duration 30  (1-30 seconds)
--model small  (small/medium/large)
```

Runs MusicGen on CPU (~10x real-time for small model). Output: WAV file.

### music_separate_stems

```
--input_file song.mp3
--model htdemucs  (htdemucs/htdemucs_ft/htdemucs_6s/mdx_extra)
--two_stems vocals  (optional: isolate one stem)
```

### music_convert_voice

```
--input_file vocals.wav
--model_name matthew.pth
--pitch 0  (-24 to +24 semitones)
--f0_method rmvpe  (rmvpe/harvest/pm)
```

### music_full_pipeline

End-to-end pipeline that chains generate → stems → voice in one call:

```
--prompt "lo-fi chill beat"
--duration 15
--separate_stems true
--voice_model matthew.pth
```

Takes 5-15 minutes depending on settings.

## Output Structure

```
~/Desktop/AI-Music/
├── *.wav                (generated tracks)
├── stems/htdemucs/      (separated stems)
│   └── track-name/
│       ├── vocals.wav
│       ├── drums.wav
│       ├── bass.wav
│       └── other.wav
└── converted/           (RVC voice conversions)
```

## Related Projects

- [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) — CLI frontend for the same tools
- [rvc-mcp](https://github.com/ExpertVagabond/rvc-mcp) — MCP server for RVC training and model management
- [music-distro](https://github.com/ExpertVagabond/music-distro) — MCP server for SoundCloud/YouTube distribution

## License

MIT — Purple Squirrel Media
