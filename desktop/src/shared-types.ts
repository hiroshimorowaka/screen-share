/** A single source in the picker's grid — one Electron `desktopCapturer`
 * source, pre-rendered to data URLs so the picker window (a separate,
 * sandboxed renderer) never needs its own access to `desktopCapturer`. */
export interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
  iconDataUrl: string | null;
}

/** What the picker window sends back once the user picks a source. */
export interface PickerChoice {
  sourceId: string;
  shareAudio: boolean;
  excludedBinaries: string[];
}

/** `PickerChoice` resolved against the real `DesktopCapturerSource` list —
 * only ever exists inside the main process. */
export interface ShareChoice {
  source: Electron.DesktopCapturerSource;
  shareAudio: boolean;
  excludedBinaries: string[];
}

/** What to link into the audio mix: either only one process's own audio
 * (sharing a specific window), or everything except the excluded
 * binaries (sharing the whole screen). */
export type AudioShareTarget =
  | { mode: 'window'; binary: string }
  | { mode: 'screen'; excludedBinaries: string[] };
