import * as path from 'node:path';
import { pathToFileURL } from 'node:url';

/** Absolute path to the bundled source-picker page, resolved from this
 * module's compiled location (`dist/features/screen-share/`). Shared so
 * `picker.ts` (which loads it) and `ipc-guard.ts` (which trusts frames
 * that came from it) agree on exactly one path. */
export const PICKER_HTML_PATH = path.join(__dirname, '..', '..', '..', 'static', 'picker.html');

/** The exact `file://` URL the picker window ends up at. `isTrustedFrame`
 * matches a sender against this instead of trusting *any* `file://` frame
 * (follow-up audit finding 13). */
export const PICKER_FILE_URL = pathToFileURL(PICKER_HTML_PATH).href;
