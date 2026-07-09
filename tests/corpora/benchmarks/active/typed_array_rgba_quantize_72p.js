const width = 128;
const height = 72;
const channels = 4;
const size = width * height * channels;
const data = typeof __imageData === "undefined" ? new Uint8Array(size) : __imageData;
let checksum = 0;

for (let index = 0; index < size; index += channels) {
    const pixel = index / channels;
    data[index] = pixel & 255;
    data[index + 1] = (pixel >> 4) & 255;
    data[index + 2] = (pixel >> 8) & 255;
    data[index + 3] = 255;
}

for (let index = 0; index < size; index += channels) {
    const luma = (data[index] * 77 + data[index + 1] * 150 + data[index + 2] * 29) >> 8;
    const bucket = luma < 64 ? 0 : luma < 128 ? 85 : luma < 192 ? 170 : 255;
    data[index] = bucket;
    data[index + 1] = bucket;
    data[index + 2] = bucket;
    checksum = (checksum + bucket) & 65535;
}

checksum + data[0] + data[size - 2]
