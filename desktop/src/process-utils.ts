import { spawn } from 'child_process';

/** Runs a command to completion and resolves with everything it wrote to
 * stdout. Never rejects — a command that fails to spawn or exits with an
 * error resolves to an empty string, since every caller here treats "no
 * output" and "command failed" the same way. */
export function runCollectingStdout(command: string, args: string[]): Promise<string> {
  return new Promise((resolve) => {
    const child = spawn(command, args);
    let output = '';
    child.stdout.on('data', (chunk: Buffer) => {
      output += chunk.toString();
    });
    child.on('close', () => resolve(output));
    child.on('error', () => resolve(''));
  });
}
