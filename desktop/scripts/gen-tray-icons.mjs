// Regenerates the tray-state icons: a filled, anti-aliased circle — green
// when idle, red while sharing — mirroring the "live dot" idea from
// easyscreenshare but with our own idle/live colours (the app's --success
// / --error). Run from the desktop/ dir: `node scripts/gen-tray-icons.mjs`.
// The two PNGs it writes are committed; this script only exists so they're
// reproducible.

import { deflateSync, crc32 } from 'node:zlib';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const SIZE = 32;
const RADIUS = SIZE / 2 - 1.5; // small inset so the edge AA isn't clipped

/** @param {[number,number,number]} rgb */
function circlePng([r, g, b]) {
  const cx = SIZE / 2 - 0.5;
  const cy = SIZE / 2 - 0.5;
  // Raw image: per row a 0 filter byte, then RGBA pixels.
  const raw = Buffer.alloc(SIZE * (1 + SIZE * 4));
  for (let y = 0; y < SIZE; y++) {
    const rowStart = y * (1 + SIZE * 4);
    raw[rowStart] = 0; // filter: none
    for (let x = 0; x < SIZE; x++) {
      const dist = Math.hypot(x - cx, y - cy);
      // 1px anti-aliased edge.
      const alpha = Math.max(0, Math.min(1, RADIUS + 0.5 - dist));
      const p = rowStart + 1 + x * 4;
      raw[p] = r;
      raw[p + 1] = g;
      raw[p + 2] = b;
      raw[p + 3] = Math.round(alpha * 255);
    }
  }

  const chunk = (type, data) => {
    const typeBuf = Buffer.from(type, 'latin1');
    const body = Buffer.concat([typeBuf, data]);
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body) >>> 0);
    return Buffer.concat([len, body, crc]);
  };

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(SIZE, 0);
  ihdr.writeUInt32BE(SIZE, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type: RGBA
  // 10..12 already zero: compression, filter, interlace

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const here = dirname(fileURLToPath(import.meta.url));
const iconsDir = join(here, '..', 'icons');

// --success #23a55a / --error #da373c from the web app's palette.
writeFileSync(join(iconsDir, 'tray-idle.png'), circlePng([0x23, 0xa5, 0x5a]));
writeFileSync(join(iconsDir, 'tray-live.png'), circlePng([0xda, 0x37, 0x3c]));
console.log('wrote icons/tray-idle.png and icons/tray-live.png');
