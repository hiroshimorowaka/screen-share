import { EventEmitter } from 'node:events';
import { afterEach, describe, expect, it, vi } from 'vitest';

const spawn = vi.hoisted(() => vi.fn());
vi.mock('node:child_process', () => ({ spawn }));

import { runCollectingStdout } from '#platform/run-command.js';

/** A minimal stand-in for a `ChildProcess`: an EventEmitter with a
 * `stdout` EventEmitter, enough for `runCollectingStdout` to wire up. */
function fakeChild() {
  const child = new EventEmitter() as EventEmitter & { stdout: EventEmitter };
  child.stdout = new EventEmitter();
  return child;
}

afterEach(() => {
  spawn.mockReset();
});

describe('runCollectingStdout', () => {
  it('resolves with everything the command wrote to stdout', async () => {
    const child = fakeChild();
    spawn.mockReturnValue(child);

    const promise = runCollectingStdout('pw-dump', ['--flag']);
    child.stdout.emit('data', Buffer.from('hello '));
    child.stdout.emit('data', Buffer.from('world'));
    child.emit('close');

    expect(await promise).toBe('hello world');
    expect(spawn).toHaveBeenCalledWith('pw-dump', ['--flag']);
  });

  it('resolves to an empty string when the command fails to spawn', async () => {
    const child = fakeChild();
    spawn.mockReturnValue(child);

    const promise = runCollectingStdout('does-not-exist', []);
    child.emit('error', new Error('ENOENT'));

    expect(await promise).toBe('');
  });

  it('resolves with whatever stdout it got even on a non-zero exit', async () => {
    const child = fakeChild();
    spawn.mockReturnValue(child);

    const promise = runCollectingStdout('xprop', ['-id', '1']);
    child.stdout.emit('data', Buffer.from('partial'));
    child.emit('close', 1);

    expect(await promise).toBe('partial');
  });
});
