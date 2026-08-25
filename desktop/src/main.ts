import { ChildProcess, spawn } from 'child_process';
import { app, BrowserWindow, desktopCapturer, ipcMain, Menu, session, Tray } from 'electron';
import * as fs from 'fs/promises';
import * as path from 'path';

// A Chromium-based app (browsers, and this app itself) plays all of its
// audio through one shared "Audio Service" subprocess, whose PID never
// matches any of the app's own window PIDs — matching audio by exact
// PID only ever works for single-process audio backends (e.g. Spotify).
// Binary name is the only identifier stable across that process split.
function resolveProcessBinary(pid: number): Promise<string | null> {
  return fs
    .readlink(`/proc/${pid}/exe`)
    .then((target) => path.basename(target))
    .catch(() => null);
}

// This app's own binary name, so its own audio playback (e.g. a member
// watching someone's share, including their own, in this same app) can
// never be swept into the mix — doing so would feed the mix's captured
// audio back into itself once shared with a watcher on this machine.
const OWN_BINARY_NAME = path.basename(process.execPath);

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let isQuitting = false;

function createMainWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });
  mainWindow.loadURL(PROD_URL);

  mainWindow.on('close', (event) => {
    if (!isQuitting) {
      event.preventDefault();
      mainWindow?.hide();
    }
  });
}

function showMainWindow(): void {
  mainWindow?.show();
  mainWindow?.focus();
}

function createTray(): void {
  const iconPath = path.join(__dirname, '..', 'icons', 'tray-icon.png');
  tray = new Tray(iconPath);
  tray.setToolTip('Screen Share');

  const menu = Menu.buildFromTemplate([
    { label: 'Abrir', click: showMainWindow },
    {
      label: 'Sair',
      click: () => {
        isQuitting = true;
        app.quit();
      },
    },
  ]);
  tray.setContextMenu(menu);
  tray.on('click', showMainWindow);
}

const MIX_SINK_NAME = 'screen_share_mix';
const MIX_SOURCE_NAME = 'screen_share_mix_out';
const MIX_PLAYBACK_PORTS = [
  `${MIX_SINK_NAME}:playback_FL`,
  `${MIX_SINK_NAME}:playback_FR`,
];

interface AudioStreamInfo {
  id: number;
  nodeName: string | null;
  pid: number | null;
  binary: string | null;
}

function runCollectingStdout(command: string, args: string[]): Promise<string> {
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

async function listAudioOutputStreams(): Promise<AudioStreamInfo[]> {
  const output = await runCollectingStdout('pw-dump', []);
  let data: unknown;
  try {
    data = JSON.parse(output);
  } catch {
    return [];
  }
  if (!Array.isArray(data)) return [];

  const streams: AudioStreamInfo[] = [];
  for (const obj of data) {
    const props = (obj as { info?: { props?: Record<string, unknown> } })?.info?.props;
    if (!props || props['media.class'] !== 'Stream/Output/Audio') continue;
    streams.push({
      id: (obj as { id: number }).id,
      nodeName: typeof props['node.name'] === 'string' ? (props['node.name'] as string) : null,
      pid:
        props['application.process.id'] !== undefined
          ? Number(props['application.process.id'])
          : null,
      binary:
        typeof props['application.process.binary'] === 'string'
          ? (props['application.process.binary'] as string)
          : null,
    });
  }
  return streams;
}

async function isNodeNamePresent(nodeName: string): Promise<boolean> {
  const output = await runCollectingStdout('pw-dump', []);
  return output.includes(`"node.name": "${nodeName}"`);
}

async function waitForNodeName(nodeName: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await isNodeNamePresent(nodeName)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for node "${nodeName}" to appear`);
}

async function listOutputPorts(nodeName: string): Promise<string[]> {
  const output = await runCollectingStdout('pw-link', ['-o']);
  const prefix = `${nodeName}:`;
  return output
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith(prefix));
}

async function linkNodeToMix(nodeName: string): Promise<void> {
  const outputs = await listOutputPorts(nodeName);
  const count = Math.min(outputs.length, MIX_PLAYBACK_PORTS.length);
  for (let i = 0; i < count; i++) {
    spawn('pw-link', [outputs[i], MIX_PLAYBACK_PORTS[i]]);
  }
}

function parseX11WindowId(sourceId: string): number | null {
  const match = sourceId.match(/^window:(\d+):/);
  return match ? parseInt(match[1], 10) : null;
}

function resolveWindowPid(x11WindowId: number): Promise<number | null> {
  return runCollectingStdout('xprop', ['-id', String(x11WindowId), '_NET_WM_PID']).then(
    (output) => {
      const match = output.match(/=\s*(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    },
  );
}

type AudioShareTarget =
  | { mode: 'window'; binary: string }
  | { mode: 'screen'; excludedBinaries: string[] };

interface AudioLoopbackSession {
  mixProcess: ChildProcess;
  pollInterval: NodeJS.Timeout;
  shouldInclude: (binary: string | null) => boolean;
}

let audioSession: AudioLoopbackSession | null = null;

// A single logical app's audio can show up as more than one PipeWire
// node sharing the same node.name — e.g. Spotify always splits into a
// named client node (which carries application.process.binary but has
// no ports of its own) and a separate adapter/follower node (which owns
// the actual linkable ports but has no binary). Deciding inclusion per
// individual stream entry meant the follower's missing binary always
// fell through the "unknown app" fail-open case, so excluding an app by
// binary silently kept linking its real ports anyway. Resolving one
// binary per node.name — from whichever entry sharing that name
// actually has it — makes the decision consistent across every node
// backing the same app.
//
// Every included node is (re-)linked on every poll rather than once:
// Chromium-based apps tear down and recreate their audio stream node
// after a period of silence (confirmed live — a paused/idle tab's node
// disappears and a fresh one appears on the next playback), so "link
// once and remember the name" left later replacement nodes never
// linked at all. Re-linking an already-connected pair just fails
// harmlessly (the exit code isn't checked), so this is safe to redo
// every second.
async function scanAndLink(): Promise<void> {
  if (!audioSession) return;
  const streams = await listAudioOutputStreams();

  const binaryByName = new Map<string, string | null>();
  for (const stream of streams) {
    if (!stream.nodeName) continue;
    if (stream.binary) {
      binaryByName.set(stream.nodeName, stream.binary);
    } else if (!binaryByName.has(stream.nodeName)) {
      binaryByName.set(stream.nodeName, null);
    }
  }

  for (const nodeName of binaryByName.keys()) {
    if (!audioSession.shouldInclude(binaryByName.get(nodeName) ?? null)) continue;
    await linkNodeToMix(nodeName);
  }
}

function shouldIncludeFor(target: AudioShareTarget): (binary: string | null) => boolean {
  const isOwnPlayback = (binary: string | null) => binary === OWN_BINARY_NAME;
  if (target.mode === 'window') {
    return (binary) => !isOwnPlayback(binary) && binary === target.binary;
  }
  return (binary) =>
    !isOwnPlayback(binary) && (!binary || !target.excludedBinaries.includes(binary));
}

async function startAudioLoopback(target: AudioShareTarget): Promise<void> {
  if (audioSession) return;
  // Computed before spawning anything: a malformed `target` must fail
  // here, not after the mix process is already running with nothing
  // left to kill it.
  const shouldInclude = shouldIncludeFor(target);
  const mixProcess = spawn('pw-loopback', [
    '--capture-props',
    `media.class=Audio/Sink node.name=${MIX_SINK_NAME} node.description="Screen Share Mix"`,
    '--playback-props',
    // The playback side must be a real Audio/Source (not left to rely
    // on the sink's implicit monitor): on stock PipeWire/WirePlumber
    // configs the monitor of an ad-hoc sink like this one is never
    // exposed as a capturable input device to browser clients, only as
    // an audiooutput — confirmed by enumerating devices in a real
    // Chromium renderer. `node.autoconnect=false` keeps WirePlumber
    // from wiring this playback stream into the real default sink,
    // which otherwise doubled every linked app's audio onto real
    // speakers the moment the mix was created (reproduced live: the
    // stream appeared connected to the hardware sink before any app was
    // even linked in).
    `media.class=Audio/Source node.name=${MIX_SOURCE_NAME} node.description="Screen Share Mix" node.passive=true node.autoconnect=false`,
  ]);
  try {
    await waitForNodeName(MIX_SINK_NAME, 3000);
  } catch (err) {
    mixProcess.kill();
    throw err;
  }

  const session: AudioLoopbackSession = {
    mixProcess,
    shouldInclude,
    pollInterval: setInterval(() => {
      void scanAndLink();
    }, 1000),
  };
  audioSession = session;
  mixProcess.on('exit', () => {
    if (audioSession === session) {
      clearInterval(session.pollInterval);
      audioSession = null;
    }
  });

  await scanAndLink();
}

function stopAudioLoopback(): void {
  if (!audioSession) return;
  clearInterval(audioSession.pollInterval);
  audioSession.mixProcess.kill();
  audioSession = null;
}

ipcMain.handle('start-audio-loopback', (_event, target: AudioShareTarget) =>
  startAudioLoopback(target),
);

ipcMain.handle('stop-audio-loopback', () => {
  stopAudioLoopback();
});

ipcMain.handle('list-audio-apps', async () => {
  const streams = await listAudioOutputStreams();
  const seen = new Set<string>();
  const apps: { binary: string; label: string }[] = [];
  for (const stream of streams) {
    if (!stream.binary || seen.has(stream.binary)) continue;
    seen.add(stream.binary);
    apps.push({ binary: stream.binary, label: stream.binary });
  }
  return apps;
});

app.on('before-quit', () => {
  stopAudioLoopback();
  isQuitting = true;
});

interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
  iconDataUrl: string | null;
}

interface PickerChoice {
  sourceId: string;
  shareAudio: boolean;
  excludedBinaries: string[];
}

interface ShareChoice {
  source: Electron.DesktopCapturerSource;
  shareAudio: boolean;
  excludedBinaries: string[];
}

function showSourcePicker(): Promise<ShareChoice | null> {
  return new Promise((resolve) => {
    void (async () => {
      const sources = await desktopCapturer.getSources({
        types: ['screen', 'window'],
        thumbnailSize: { width: 300, height: 200 },
        fetchWindowIcons: true,
      });

      const pickerSources: PickerSource[] = sources.map((s) => ({
        id: s.id,
        name: s.name,
        thumbnailDataUrl: s.thumbnail.toDataURL(),
        iconDataUrl: s.appIcon && !s.appIcon.isEmpty() ? s.appIcon.toDataURL() : null,
      }));

      const pickerWindow = new BrowserWindow({
        width: 1000,
        height: 720,
        parent: mainWindow ?? undefined,
        frame: false,
        transparent: true,
        resizable: true,
        minWidth: 640,
        minHeight: 480,
        skipTaskbar: true,
        webPreferences: {
          preload: path.join(__dirname, 'preload.js'),
        },
      });

      let settled = false;
      const settle = (choice: PickerChoice | null) => {
        if (settled) return;
        settled = true;
        if (!choice) {
          resolve(null);
        } else {
          const source = sources.find((s) => s.id === choice.sourceId) ?? null;
          resolve(
            source
              ? {
                  source,
                  shareAudio: choice.shareAudio,
                  excludedBinaries: choice.excludedBinaries,
                }
              : null,
          );
        }
        if (!pickerWindow.isDestroyed()) pickerWindow.close();
      };

      ipcMain.once('picker:selected', (_event, choice: PickerChoice) => settle(choice));
      pickerWindow.on('closed', () => settle(null));

      // Delay arming "click outside closes it" slightly so the window
      // manager focusing this new window doesn't itself trigger a blur.
      setTimeout(() => {
        pickerWindow.on('blur', () => settle(null));
      }, 300);

      await pickerWindow.loadFile(
        path.join(__dirname, '..', 'static', 'picker.html'),
      );
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}

async function resolveAudioTarget(chosen: ShareChoice): Promise<AudioShareTarget | null> {
  if (chosen.source.id.startsWith('window:')) {
    const x11Id = parseX11WindowId(chosen.source.id);
    if (x11Id === null) return null;
    const pid = await resolveWindowPid(x11Id);
    if (pid === null) return null;
    const binary = await resolveProcessBinary(pid);
    if (binary === null) return null;
    return { mode: 'window', binary };
  }
  return { mode: 'screen', excludedBinaries: chosen.excludedBinaries };
}

app.whenReady().then(() => {
  Menu.setApplicationMenu(null);
  createMainWindow();
  createTray();

  session.defaultSession.setDisplayMediaRequestHandler(
    async (_request, callback) => {
      const chosen = await showSourcePicker();
      if (!chosen) {
        callback({});
        return;
      }
      if (chosen.shareAudio) {
        const target = await resolveAudioTarget(chosen);
        if (target) {
          try {
            await startAudioLoopback(target);
          } catch {
            // Proceed with video-only rather than failing the whole share.
          }
        }
      }
      callback({ video: chosen.source });
    },
  );
});
