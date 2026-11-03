// Генерация placeholder-иконки 1024x1024 RGBA (оранжевый скруглённый квадрат + «+»)
// без внешних зависимостей: чистый PNG-энкодер на node:zlib.
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

const W = 1024;
const H = 1024;
const R = 180; // скругление углов
const ACCENT = [232, 163, 61]; // --accent из app.css

function crc32(buf) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let c = ~0;
  for (let i = 0; i < buf.length; i++) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return ~c;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body) >>> 0);
  return Buffer.concat([len, body, crc]);
}

// скруглённый прямоугольник: внутри — да, за радиусом — нет
function inside(x, y) {
  const inX = x >= R && x <= W - R;
  const inY = y >= R && y <= H - R;
  if (inX || inY) return inX || inY;
  const dx = Math.min(x, W - x) - R;
  const dy = Math.min(y, H - y) - R;
  return dx * dx + dy * dy <= R * R;
}

// штрих-код «плюса»
const barHalf = 60; // полутолщина
const arm = 250; // полудлина плеча
const cx = W / 2;
const cy = H / 2;
function plus(x, y) {
  const dx = x - cx;
  const dy = y - cy;
  return (
    (Math.abs(dy) <= barHalf && Math.abs(dx) <= arm) ||
    (Math.abs(dx) <= barHalf && Math.abs(dy) <= arm)
  );
}

const raw = Buffer.alloc(H * (1 + W * 4));
let o = 0;
for (let y = 0; y < H; y++) {
  raw[o++] = 0; // filter: none
  for (let x = 0; x < W; x++) {
    if (inside(x, y)) {
      const white = plus(x, y);
      const [r, g, b] = white ? [255, 255, 255] : ACCENT;
      raw[o++] = r;
      raw[o++] = g;
      raw[o++] = b;
      raw[o++] = 255;
    } else {
      raw[o++] = 0;
      raw[o++] = 0;
      raw[o++] = 0;
      raw[o++] = 0;
    }
  }
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
// compression/filter/interleave = 0

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

mkdirSync("src-tauri/icons", { recursive: true });
writeFileSync("src-tauri/icons/icon.png", png);
writeFileSync("app-icon.png", png);
console.log("OK:", png.length, "bytes -> src-tauri/icons/icon.png");
