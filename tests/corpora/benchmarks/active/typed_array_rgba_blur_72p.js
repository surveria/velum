const width = 128;
const height = 72;
const channels = 4;
const size = width * height * channels;
const data = typeof __imageData === "undefined" ? new Uint8Array(size) : __imageData;
const output = new Uint8Array(size);
let checksum = 0;

for (let index = 0; index < size; index += channels) {
    const pixel = index / channels;
    data[index] = pixel & 255;
    data[index + 1] = (pixel >> 3) & 255;
    data[index + 2] = (pixel >> 6) & 255;
    data[index + 3] = 255;
}

for (let y = 1; y < height - 1; y++) {
    const row = y * width * channels;
    for (let x = 1; x < width - 1; x++) {
        const offset = row + x * channels;
        const top = offset - width * channels;
        const bottom = offset + width * channels;
        const red = (
            data[top - channels] + data[top] + data[top + channels] +
            data[offset - channels] + data[offset] + data[offset + channels] +
            data[bottom - channels] + data[bottom] + data[bottom + channels]
        ) / 9;
        const green = (
            data[top - channels + 1] + data[top + 1] + data[top + channels + 1] +
            data[offset - channels + 1] + data[offset + 1] + data[offset + channels + 1] +
            data[bottom - channels + 1] + data[bottom + 1] + data[bottom + channels + 1]
        ) / 9;
        const blue = (
            data[top - channels + 2] + data[top + 2] + data[top + channels + 2] +
            data[offset - channels + 2] + data[offset + 2] + data[offset + channels + 2] +
            data[bottom - channels + 2] + data[bottom + 2] + data[bottom + channels + 2]
        ) / 9;
        output[offset] = red;
        output[offset + 1] = green;
        output[offset + 2] = blue;
        output[offset + 3] = 255;
        checksum = (checksum + output[offset] + output[offset + 1] + output[offset + 2]) & 65535;
    }
}

checksum + output[channels * (width + 1)]
