const width = 80;
const height = 45;
const channels = 4;
const size = width * height * channels;
const data = typeof __imageData === "undefined" ? new Uint8Array(size) : __imageData;
const output = new Uint8Array(size);
let checksum = 0;

for (let index = 0; index < size; index += channels) {
    const pixel = index / channels;
    data[index] = pixel & 255;
    data[index + 1] = (pixel >> 2) & 255;
    data[index + 2] = (pixel >> 5) & 255;
    data[index + 3] = 255;
}

for (let y = 1; y < height - 1; y++) {
    const row = y * width * channels;
    for (let x = 1; x < width - 1; x++) {
        const offset = row + x * channels;
        const top = offset - width * channels;
        const bottom = offset + width * channels;
        const red = data[offset] * 5 - data[top] - data[bottom] -
            data[offset - channels] - data[offset + channels];
        const green = data[offset + 1] * 5 - data[top + 1] - data[bottom + 1] -
            data[offset - channels + 1] - data[offset + channels + 1];
        const blue = data[offset + 2] * 5 - data[top + 2] - data[bottom + 2] -
            data[offset - channels + 2] - data[offset + channels + 2];
        output[offset] = red < 0 ? 0 : red > 255 ? 255 : red;
        output[offset + 1] = green < 0 ? 0 : green > 255 ? 255 : green;
        output[offset + 2] = blue < 0 ? 0 : blue > 255 ? 255 : blue;
        output[offset + 3] = 255;
        checksum = (checksum + output[offset] + output[offset + 1] + output[offset + 2]) & 65535;
    }
}

checksum + output[channels * (width + 1)]
