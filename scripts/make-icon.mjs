/**
 * Generates src-tauri/icons/app-icon.png (1024×1024, transparent) — the source
 * image for `pnpm tauri icon`, which derives every platform icon from it.
 *
 * Pure Node (zlib only) so the repo needs no image tooling. Draws a dark
 * rounded plate with the four-light signal; the "working" light is lit.
 */
import { deflateSync } from 'node:zlib';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SIZE = 1024;
const OUT = resolve(dirname(fileURLToPath(import.meta.url)), '../src-tauri/icons/app-icon.png');

const px = new Uint8Array(SIZE * SIZE * 4);

/** Composite an RGBA sample "over" the pixel at (x, y). */
function put(x, y, r, g, b, a) {
  if (x < 0 || y < 0 || x >= SIZE || y >= SIZE || a <= 0) return;
  const i = (y * SIZE + x) * 4;
  const sa = a / 255;
  const da = px[i + 3] / 255;
  const oa = sa + da * (1 - sa);
  if (oa === 0) return;
  const blend = (s, d) => Math.round((s * sa + d * da * (1 - sa)) / oa);
  px[i] = blend(r, px[i]);
  px[i + 1] = blend(g, px[i + 1]);
  px[i + 2] = blend(b, px[i + 2]);
  px[i + 3] = Math.round(oa * 255);
}

const coverage = (signedDistance) => Math.min(1, Math.max(0, 0.5 - signedDistance));

function sdRoundRect(x, y, cx, cy, hw, hh, r) {
  const qx = Math.abs(x - cx) - hw + r;
  const qy = Math.abs(y - cy) - hh + r;
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r;
}

function fillRoundRect(cx, cy, hw, hh, r, [R, G, B, A]) {
  for (let y = Math.floor(cy - hh) - 1; y <= Math.ceil(cy + hh) + 1; y++) {
    for (let x = Math.floor(cx - hw) - 1; x <= Math.ceil(cx + hw) + 1; x++) {
      const c = coverage(sdRoundRect(x + 0.5, y + 0.5, cx, cy, hw, hh, r));
      if (c > 0) put(x, y, R, G, B, A * c);
    }
  }
}

function fillCircle(cx, cy, rad, [R, G, B, A]) {
  for (let y = Math.floor(cy - rad) - 1; y <= Math.ceil(cy + rad) + 1; y++) {
    for (let x = Math.floor(cx - rad) - 1; x <= Math.ceil(cx + rad) + 1; x++) {
      const c = coverage(Math.hypot(x + 0.5 - cx, y + 0.5 - cy) - rad);
      if (c > 0) put(x, y, R, G, B, A * c);
    }
  }
}

function glow(cx, cy, rad, spread, [R, G, B]) {
  const outer = rad + spread;
  for (let y = Math.floor(cy - outer); y <= Math.ceil(cy + outer); y++) {
    for (let x = Math.floor(cx - outer); x <= Math.ceil(cx + outer); x++) {
      const d = Math.hypot(x + 0.5 - cx, y + 0.5 - cy) - rad;
      if (d > 0 && d < spread) {
        const t = 1 - d / spread;
        put(x, y, R, G, B, 255 * 0.4 * t * t);
      }
    }
  }
}

// Plate
fillRoundRect(512, 512, 420, 420, 210, [22, 22, 27, 255]);
fillRoundRect(512, 512, 419, 419, 209, [255, 255, 255, 12]); // faint inner highlight

// Four lights, top → bottom: working (lit), waiting, done, error
const lights = [
  [52, 211, 153],
  [245, 165, 36],
  [244, 244, 245],
  [244, 63, 94],
];
lights.forEach((rgb, i) => {
  const cy = 512 + (i - 1.5) * 190;
  if (i === 0) glow(512, cy, 64, 90, rgb);
  fillCircle(512, cy, 64, [...rgb, i === 0 ? 255 : 80]);
});

// --- PNG encoding -----------------------------------------------------------
const CRC_TABLE = new Int32Array(256);
for (let n = 0; n < 256; n++) {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  CRC_TABLE[n] = c;
}
function crc32(buf) {
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeAndData = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(typeAndData));
  return Buffer.concat([len, typeAndData, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0; // filter: none
  raw.set(px.subarray(y * SIZE * 4, (y + 1) * SIZE * 4), y * (SIZE * 4 + 1) + 1);
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, png);
console.log(`wrote ${OUT} (${png.length} bytes)`);
