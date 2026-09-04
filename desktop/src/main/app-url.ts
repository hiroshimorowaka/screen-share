const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

/** The web app the desktop shell wraps. Overridable via `SCREEN_SHARE_URL`
 * so a dev can point at a local `cargo leptos serve` and the `_electron`
 * E2E can load a deterministic page. Unset in every shipped build. */
export const APP_URL = process.env.SCREEN_SHARE_URL ?? PROD_URL;

/** Scheme + host + port of {@link APP_URL}. The single origin the renderer
 * is allowed to navigate to and the only one whose frames may drive the
 * privileged IPC bridges. */
export const APP_ORIGIN = new URL(APP_URL).origin;
