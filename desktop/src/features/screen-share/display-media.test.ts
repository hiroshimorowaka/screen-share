import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ShareChoice } from '#ipc/types.js';

const setDisplayMediaRequestHandler = vi.hoisted(() => vi.fn());
const showSourcePicker = vi.hoisted(() => vi.fn());
const armAudioCaptureGrant = vi.hoisted(() => vi.fn());
const backend = vi.hoisted(() => ({
  startAudioLoopback: vi.fn(async () => {}),
  resolveAudioTarget: vi.fn(async () => ({ mode: 'screen', excludedBinaries: [] })),
}));
const loadAudioBackend = vi.hoisted(() => vi.fn(async () => backend));

vi.mock('electron', () => ({
  session: { defaultSession: { setDisplayMediaRequestHandler } },
}));
vi.mock('#features/screen-share/picker.js', () => ({ showSourcePicker }));
vi.mock('#features/audio-share/backend.js', () => ({ loadAudioBackend }));
vi.mock('#main/permissions.js', () => ({ armAudioCaptureGrant }));

import { registerDisplayMediaHandler } from '#features/screen-share/display-media.js';

const SOURCE = { id: 'screen:0' } as unknown as ShareChoice['source'];
const choice = (shareAudio: boolean): ShareChoice => ({
  source: SOURCE,
  shareAudio,
  excludedBinaries: [],
});

/** Runs the registered display-media callback and returns what it passed
 * to `callback`. */
async function invokeHandler(): Promise<unknown> {
  const handler = setDisplayMediaRequestHandler.mock.calls.at(-1)?.[0];
  return new Promise((resolve) => {
    void handler({}, resolve);
  });
}

beforeEach(() => {
  setDisplayMediaRequestHandler.mockReset();
  showSourcePicker.mockReset();
  armAudioCaptureGrant.mockReset();
  loadAudioBackend.mockClear();
  backend.startAudioLoopback.mockClear();
  backend.resolveAudioTarget.mockClear();
  backend.resolveAudioTarget.mockResolvedValue({ mode: 'screen', excludedBinaries: [] });
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

describe('registerDisplayMediaHandler', () => {
  it('registers the handler synchronously, without touching the audio backend', () => {
    registerDisplayMediaHandler();
    expect(setDisplayMediaRequestHandler).toHaveBeenCalledOnce();
    expect(loadAudioBackend).not.toHaveBeenCalled();
  });

  it('grants nothing when the picker is dismissed', async () => {
    registerDisplayMediaHandler();
    showSourcePicker.mockResolvedValue(null);
    expect(await invokeHandler()).toEqual({});
    expect(loadAudioBackend).not.toHaveBeenCalled();
  });

  it('shares video only, never loading the backend, when audio was not requested', async () => {
    registerDisplayMediaHandler();
    showSourcePicker.mockResolvedValue(choice(false));
    expect(await invokeHandler()).toEqual({ video: SOURCE });
    expect(loadAudioBackend).not.toHaveBeenCalled();
  });

  it('still shares video when the audio backend fails to load (the Windows regression)', async () => {
    registerDisplayMediaHandler();
    showSourcePicker.mockResolvedValue(choice(true));
    loadAudioBackend.mockRejectedValueOnce(
      new Error('windows-audio: failed to load native binding'),
    );

    expect(await invokeHandler()).toEqual({ video: SOURCE });
    expect(armAudioCaptureGrant).not.toHaveBeenCalled();
    expect(console.error).toHaveBeenCalledOnce();
  });

  it('starts the loopback and arms the capture grant when audio is available', async () => {
    registerDisplayMediaHandler();
    showSourcePicker.mockResolvedValue(choice(true));

    expect(await invokeHandler()).toEqual({ video: SOURCE });
    expect(backend.startAudioLoopback).toHaveBeenCalledWith({
      mode: 'screen',
      excludedBinaries: [],
    });
    expect(armAudioCaptureGrant).toHaveBeenCalledOnce();
  });

  it('shares video only when no audio target resolves (audio-less source)', async () => {
    registerDisplayMediaHandler();
    showSourcePicker.mockResolvedValue(choice(true));
    backend.resolveAudioTarget.mockResolvedValueOnce(null);

    expect(await invokeHandler()).toEqual({ video: SOURCE });
    expect(backend.startAudioLoopback).not.toHaveBeenCalled();
    expect(armAudioCaptureGrant).not.toHaveBeenCalled();
  });
});
