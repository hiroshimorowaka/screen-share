# Electron Desktop Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Tauri desktop shell (blocked by a WebKitGTK screen-capture bug on Linux, see `docs/superpowers/specs/2026-08-21-tauri-screen-share-black-video-investigation.md`) with an Electron shell that opens the production site in a native window, keeps the tray/close-to-hide behavior, and makes screen sharing actually work by packaging real Chromium.

**Architecture:** `desktop/src-tauri/` is deleted. `desktop/` becomes an Electron + TypeScript project managed with `pnpm`, independent of the root Rust crate (no workspace). The main window loads `https://screen-share-h0rb5w.fly.dev/` as an external URL — no web assets are bundled. The one genuinely new piece: Electron doesn't show a native screen/window picker on its own, so the main process implements `session.setDisplayMediaRequestHandler` backed by a small first-party picker window.

**Tech Stack:** Electron 43.4.1, TypeScript 7.0.2, `@types/node` 26.2.0, `pnpm`. Linux only for this plan.

## Global Constraints

- The root crate (`screen-share`, repo root) is not modified by this plan.
- `desktop/` is a standalone project (its own `package.json`/`pnpm-lock.yaml`), not a Cargo workspace member and not linked to the root crate.
- The window loads `https://screen-share-h0rb5w.fly.dev/` exactly — no bundled/local frontend.
- Target platform: Linux only. No Windows-specific code.
- Closing the main window (X) hides it; only "Sair" in the tray menu quits for real.
- No installers, code signing, or auto-update in this plan.
- The app never requests audio in `getDisplayMedia()` today (per `CLAUDE.md`: "não há áudio ainda") — the display-media handler must not assume `audioRequested` is ever `true`, only handle it if the request actually asks for it.
- The app icon is the placeholder already generated for the Tauri shell (`desktop/src-tauri/icons/32x32.png`) — reused, not redesigned.
- No browser-automation test harness for this layer — every task ends with a manual verification checklist.

---

### Task 1: Remove the Tauri project, scaffold Electron + TypeScript, open the production site

**Files:**
- Delete: `desktop/src-tauri/` (entire directory)
- Create: `desktop/icons/tray-icon.png` (copied from the old Tauri icon before deleting it)
- Create: `desktop/package.json`
- Create: `desktop/tsconfig.json`
- Create: `desktop/.gitignore`
- Create: `desktop/src/main.ts`

**Interfaces:**
- Produces: a runnable `desktop/` Electron project, `pnpm start` opens a window loading the production site. Task 2 extends `desktop/src/main.ts`'s `app.whenReady()` block with tray/close-to-hide logic. Task 3 further extends it with the display-media handler and adds sibling files.

- [ ] **Step 1: Copy the tray icon out before deleting the Tauri project**

```bash
mkdir -p desktop/icons
cp desktop/src-tauri/icons/32x32.png desktop/icons/tray-icon.png
```

- [ ] **Step 2: Delete the Tauri project**

```bash
git rm -r desktop/src-tauri
```

- [ ] **Step 3: Write `desktop/package.json`**

```json
{
  "name": "screen-share-desktop",
  "version": "0.1.0",
  "private": true,
  "main": "dist/main.js",
  "scripts": {
    "build": "tsc",
    "start": "tsc && electron ."
  },
  "devDependencies": {
    "@types/node": "^26.2.0",
    "electron": "^43.4.1",
    "typescript": "^7.0.2"
  }
}
```

- [ ] **Step 4: Write `desktop/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["src"]
}
```

(TypeScript 7's compiler removed the legacy `moduleResolution: "Node"` —
`module`/`moduleResolution` must both be `"Node16"` together for a
CommonJS Node/Electron project like this one.)

- [ ] **Step 5: Write `desktop/.gitignore`**

```
node_modules/
dist/
```

- [ ] **Step 6: Write a minimal `desktop/src/main.ts`**

Just enough to open the window — tray, close-to-hide, and the display-media
handler are added in later tasks, so this step's own verification is
isolated to "does a window open and load the site."

```typescript
import { app, BrowserWindow } from 'electron';

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

function createMainWindow(): void {
  const mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
  });
  mainWindow.loadURL(PROD_URL);
}

app.whenReady().then(() => {
  createMainWindow();
});
```

- [ ] **Step 7: Install dependencies**

```bash
cd desktop && pnpm install
```

- [ ] **Step 8: Run it and verify manually**

```bash
cd desktop && pnpm start
```

Manually confirm the window opens, titled "Screen Share", and loads the
production room-share UI (not a blank page or a connection error). Stop
with Ctrl+C once confirmed.

- [ ] **Step 9: Commit**

```bash
git add desktop/icons/tray-icon.png desktop/package.json desktop/tsconfig.json \
  desktop/.gitignore desktop/src/main.ts desktop/pnpm-lock.yaml
git commit -m "feat(desktop): replace Tauri shell with Electron, load production site"
```

(The `git rm -r desktop/src-tauri` from Step 2 is staged automatically —
this commit both removes the old project and adds the new one.)

---

### Task 2: Tray icon, close-to-hide, and quit

**Files:**
- Modify: `desktop/src/main.ts`

**Interfaces:**
- Consumes: `createMainWindow()` and the `mainWindow` it creates, from Task 1.
- Produces: a module-level `mainWindow: BrowserWindow | null` and `isQuitting: boolean` that Task 3's display-media handler reads to parent its picker window.

- [ ] **Step 1: Replace `desktop/src/main.ts` with the full tray/close-to-hide version**

```typescript
import { app, BrowserWindow, Tray, Menu } from 'electron';
import * as path from 'path';

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let isQuitting = false;

function createMainWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
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

app.on('before-quit', () => {
  isQuitting = true;
});

app.whenReady().then(() => {
  createMainWindow();
  createTray();
});
```

- [ ] **Step 2: Rebuild and verify manually**

```bash
cd desktop && pnpm start
```

Manually confirm:
- A tray icon appears in the system tray/status area.
- Clicking the window's X button hides it (disappears from the taskbar)
  but the process keeps running (`ps aux | grep electron` still shows
  it).
- Clicking the tray icon, or choosing "Abrir" from its menu, brings the
  window back with its previous state intact.
- Choosing "Sair" from the tray menu actually ends the process (it
  disappears from `ps aux | grep electron` and the tray icon goes away).

Stop the process once confirmed (via "Sair", or Ctrl+C if still visible
in the terminal).

- [ ] **Step 3: Commit**

```bash
git add desktop/src/main.ts
git commit -m "feat(desktop): add tray icon with close-to-hide behavior"
```

---

### Task 3: Screen/window picker for `getDisplayMedia()`

**Files:**
- Create: `desktop/src/preload.ts`
- Create: `desktop/static/picker.html`
- Create: `desktop/static/picker.js`
- Modify: `desktop/src/main.ts`

**Interfaces:**
- Consumes: `mainWindow` (as the picker window's `parent`) from Task 2.
- Produces: nothing consumed by further tasks — this is the last task in
  this plan.

Electron does not show a native screen/window picker on its own the way
a browser tab does — `session.setDisplayMediaRequestHandler` hands the
app full control over what gets offered. This task adds a small,
first-party picker window: a grid of thumbnails (via
`desktopCapturer.getSources`), click one to share it.

Cancellation is a known rough edge in Electron's own API (see
`electron/electron#47980` — throwing an exception from the handler to
signal cancellation is reported to hang the page's `getDisplayMedia()`
call and, on Linux, sometimes even breaks subsequent share attempts
until the app restarts). This plan avoids throwing entirely: on
cancellation, the handler calls `callback({})` — the documented,
type-safe way to signal "no stream" since `video` is an optional field
on the callback's argument. Step 5 below explicitly re-tests
cancellation for this reason; if it turns out `callback({})` also hangs
on this machine, that's a "stop and report back" situation, not
something to silently work around with more guessing.

- [ ] **Step 1: Write `desktop/src/preload.ts`**

```typescript
import { contextBridge, ipcRenderer } from 'electron';

interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
}

contextBridge.exposeInMainWorld('picker', {
  onSources: (callback: (sources: PickerSource[]) => void) => {
    ipcRenderer.on('picker:sources', (_event, sources: PickerSource[]) => {
      callback(sources);
    });
  },
  select: (id: string) => {
    ipcRenderer.send('picker:selected', id);
  },
});
```

- [ ] **Step 2: Write `desktop/static/picker.html`**

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Escolha o que compartilhar</title>
    <style>
      body {
        font-family: sans-serif;
        margin: 0;
        padding: 16px;
        background: #1e1e1e;
        color: #eee;
      }
      h1 {
        font-size: 16px;
        margin: 0 0 12px;
      }
      #grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 12px;
      }
      .source {
        cursor: pointer;
        border: 2px solid transparent;
        border-radius: 6px;
        padding: 6px;
        text-align: center;
      }
      .source:hover {
        border-color: #6c8cff;
      }
      .source img {
        width: 100%;
        border-radius: 4px;
        display: block;
      }
      .source span {
        display: block;
        margin-top: 6px;
        font-size: 12px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
    </style>
  </head>
  <body>
    <h1>O que você quer compartilhar?</h1>
    <div id="grid"></div>
    <script src="picker.js"></script>
  </body>
</html>
```

- [ ] **Step 3: Write `desktop/static/picker.js`**

```javascript
window.picker.onSources((sources) => {
  const grid = document.getElementById('grid');
  for (const source of sources) {
    const el = document.createElement('div');
    el.className = 'source';
    el.innerHTML =
      '<img src="' + source.thumbnailDataUrl + '" alt="">' +
      '<span>' + source.name + '</span>';
    el.addEventListener('click', () => {
      window.picker.select(source.id);
    });
    grid.appendChild(el);
  }
});
```

- [ ] **Step 4: Extend `desktop/src/main.ts` with the display-media handler**

Add these imports to the existing import line and add the new code
before `app.whenReady()`:

```typescript
import { app, BrowserWindow, Tray, Menu, session, desktopCapturer, ipcMain } from 'electron';
```

```typescript
interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
}

function showSourcePicker(): Promise<Electron.DesktopCapturerSource | null> {
  return new Promise((resolve) => {
    void (async () => {
      const sources = await desktopCapturer.getSources({
        types: ['screen', 'window'],
        thumbnailSize: { width: 300, height: 200 },
      });

      const pickerSources: PickerSource[] = sources.map((s) => ({
        id: s.id,
        name: s.name,
        thumbnailDataUrl: s.thumbnail.toDataURL(),
      }));

      const pickerWindow = new BrowserWindow({
        width: 640,
        height: 480,
        parent: mainWindow ?? undefined,
        modal: true,
        title: 'Escolha o que compartilhar',
        webPreferences: {
          preload: path.join(__dirname, 'preload.js'),
        },
      });

      let settled = false;
      const settle = (id: string | null) => {
        if (settled) return;
        settled = true;
        resolve(id ? sources.find((s) => s.id === id) ?? null : null);
        if (!pickerWindow.isDestroyed()) pickerWindow.close();
      };

      ipcMain.once('picker:selected', (_event, id: string) => settle(id));
      pickerWindow.on('closed', () => settle(null));

      await pickerWindow.loadFile(
        path.join(__dirname, '..', 'static', 'picker.html'),
      );
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}
```

Add this inside `app.whenReady().then(() => { ... })`, after
`createTray();`:

```typescript
  session.defaultSession.setDisplayMediaRequestHandler(
    async (_request, callback) => {
      const chosen = await showSourcePicker();
      callback(chosen ? { video: chosen } : {});
    },
  );
```

- [ ] **Step 5: Rebuild and verify manually**

```bash
cd desktop && pnpm start
```

Manually confirm, entering a room first:

- Clicking "compartilhar tela" opens the picker window with thumbnails
  of your screen(s) and open windows.
- Clicking a thumbnail closes the picker and actually starts sharing —
  video visible in your own preview.
- A second person, watching from an ordinary browser tab in the same
  room, sees the shared video too. This is the thing the Tauri shell
  could never do — if it fails here, stop and report back rather than
  guessing further fixes.
- Closing the picker window without clicking anything cancels the share
  request cleanly: no hang, no stuck spinner, and the page is left in a
  state where clicking "compartilhar tela" again works normally (this is
  the specific Linux failure mode reported in
  `electron/electron#47980` — confirm it does NOT happen here).

Stop the process once confirmed.

- [ ] **Step 6: Commit**

```bash
git add desktop/src/main.ts desktop/src/preload.ts desktop/static
git commit -m "feat(desktop): add screen/window picker for getDisplayMedia"
```

---

## Definition of done

All three tasks' manual verification checklists pass on your Linux
machine, including screen sharing actually rendering video (not black)
both locally and for a remote viewer. Windows support, audio, installers,
and final branding remain out of scope per
`docs/superpowers/specs/2026-08-21-electron-desktop-shell-design.md`.
