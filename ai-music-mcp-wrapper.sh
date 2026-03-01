#!/bin/bash
# AI Music MCP Server wrapper
# Sets environment variables and launches the MCP server
export MUSIC_STUDIO_DIR="/Volumes/Virtual Server/projects/ai-music-studio"
export RVC_DIR="/Volumes/Virtual Server/projects/ai-music-rvc"
export MUSIC_OUTPUT_DIR="$HOME/Desktop/AI-Music"
exec node "/Volumes/Virtual Server/projects/ai-music-mcp/typescript/build/index.js"
