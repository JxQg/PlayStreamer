import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Resvg } from '@resvg/resvg-js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.join(root, 'src-tauri/icons');
const source = await readFile(path.join(root, 'public/brand/playstreamer.svg'));
await mkdir(output, { recursive: true });
await writeFile(path.join(output, 'icon.svg'), source);

// Every size uses the same vector master, including the Windows notification area.
const render = size => new Resvg(source, { fitTo: { mode: 'width', value: size } }).render().asPng();
await writeFile(path.join(output, 'icon.png'), render(256));
await writeFile(path.join(output, 'tray.png'), render(32));
const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
const images = sizes.map(render);
const directory = Buffer.alloc(6 + sizes.length * 16);
directory.writeUInt16LE(1, 2);
directory.writeUInt16LE(sizes.length, 4);
let offset = directory.length;
images.forEach((bytes, index) => {
  const entry = 6 + index * 16;
  directory[entry] = sizes[index] % 256;
  directory[entry + 1] = sizes[index] % 256;
  directory.writeUInt16LE(1, entry + 4);
  directory.writeUInt16LE(32, entry + 6);
  directory.writeUInt32LE(bytes.length, entry + 8);
  directory.writeUInt32LE(offset, entry + 12);
  offset += bytes.length;
});
await writeFile(path.join(output, 'icon.ico'), Buffer.concat([directory, ...images]));
console.log('Generated application, taskbar and tray icons from public/brand/playstreamer.svg.');
