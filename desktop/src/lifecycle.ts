import { app } from 'electron';

/** Whether the app is shutting down for real, vs. just having its main
 * window closed — `main-window.ts`'s close handler checks this to decide
 * between hiding (tray behavior) and letting the close proceed. */
let quitting = false;

export function isQuitting(): boolean {
  return quitting;
}

/** Marks that the app is actually quitting, not just having its main
 * window closed. Called from every path that can end the process: the
 * tray's "Sair" (via `requestQuit`) and `before-quit` as a catch-all for
 * anything else (Cmd+Q, the OS shutting the app down). */
export function markQuitting(): void {
  quitting = true;
}

/** The tray's own quit action — marks intent first so the main window's
 * close handler lets this one through instead of hiding it like every
 * other close. */
export function requestQuit(): void {
  markQuitting();
  app.quit();
}
