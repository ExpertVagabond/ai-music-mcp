import { spawn } from "node:child_process";
import { resolve } from "node:path";

export interface SubprocessResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  durationMs: number;
}

export function runPython(
  venvPath: string,
  args: string[],
  options?: {
    cwd?: string;
    timeoutMs?: number;
    env?: Record<string, string>;
  }
): Promise<SubprocessResult> {
  return new Promise((resolve_p, reject) => {
    const pythonBin = resolve(venvPath, "bin/python");
    const startTime = Date.now();
    const timeout = options?.timeoutMs ?? 300_000;

    const proc = spawn(pythonBin, args, {
      cwd: options?.cwd,
      env: {
        ...process.env,
        TOKENIZERS_PARALLELISM: "false",
        PYTORCH_ENABLE_MPS_FALLBACK: "1",
        ...options?.env,
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });

    proc.stderr.on("data", (data: Buffer) => {
      const chunk = data.toString();
      stderr += chunk;
      process.stderr.write(`[ai-music] ${chunk}`);
    });

    const timer = setTimeout(() => {
      proc.kill("SIGTERM");
      setTimeout(() => {
        try {
          proc.kill("SIGKILL");
        } catch {
          // already dead
        }
      }, 5000);
      reject(new Error(`Process timed out after ${timeout}ms`));
    }, timeout);

    proc.on("close", (code) => {
      clearTimeout(timer);
      resolve_p({
        stdout: stdout.trim(),
        stderr: stderr.trim(),
        exitCode: code ?? 1,
        durationMs: Date.now() - startTime,
      });
    });

    proc.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
  });
}
