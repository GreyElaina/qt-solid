import { deflateSync } from "node:zlib"

/**
 * Encode raw RGBA8 straight-alpha pixel data into a PNG data URL.
 */
export function encodeRgbaPng(rgba: Uint8Array, width: number, height: number): string {
  const rowBytes = 1 + width * 4
  const raw = Buffer.alloc(rowBytes * height)
  for (let y = 0; y < height; y++) {
    const dstOffset = y * rowBytes
    raw[dstOffset] = 0 // filter: None
    const srcOffset = y * width * 4
    for (let i = 0; i < width * 4; i++) {
      raw[dstOffset + 1 + i] = rgba[srcOffset + i]!
    }
  }

  const compressed = deflateSync(raw)
  const chunks: Buffer[] = []

  // Signature
  chunks.push(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))

  // IHDR
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8   // bit depth
  ihdr[9] = 6   // color type: RGBA
  ihdr[10] = 0  // compression
  ihdr[11] = 0  // filter
  ihdr[12] = 0  // interlace
  chunks.push(pngChunk("IHDR", ihdr))

  // IDAT
  chunks.push(pngChunk("IDAT", compressed))

  // IEND
  chunks.push(pngChunk("IEND", Buffer.alloc(0)))

  const png = Buffer.concat(chunks)
  return `data:image/png;base64,${png.toString("base64")}`
}

function pngChunk(type: string, data: Buffer): Buffer {
  const buf = Buffer.alloc(4 + 4 + data.length + 4)
  buf.writeUInt32BE(data.length, 0)
  buf.write(type, 4, 4, "ascii")
  data.copy(buf, 8)
  buf.writeUInt32BE(crc32(buf.subarray(4, 8 + data.length)), 8 + data.length)
  return buf
}

const crcTable = new Uint32Array(256)
for (let n = 0; n < 256; n++) {
  let c = n
  for (let k = 0; k < 8; k++) {
    c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1)
  }
  crcTable[n] = c
}

function crc32(data: Uint8Array): number {
  let crc = 0xFFFFFFFF
  for (let i = 0; i < data.length; i++) {
    crc = crcTable[(crc ^ data[i]!) & 0xFF]! ^ (crc >>> 8)
  }
  return (crc ^ 0xFFFFFFFF) >>> 0
}
