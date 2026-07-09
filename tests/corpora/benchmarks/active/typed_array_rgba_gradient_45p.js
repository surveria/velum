const width = 80;
const height = 45;
const channels = 4;
const size = width * height * channels;
const data = typeof __imageData === "undefined" ? new Uint8Array(size) : __imageData;
let checksum = 0;

for (let y = 0; y < height; y++) {
    const row = y * width * channels;
    for (let x = 0; x < width; x++) {
        const offset = row + x * channels;
        const red = x & 255;
        const green = y & 255;
        const blue = (x + y) & 255;
        data[offset] = red;
        data[offset + 1] = green;
        data[offset + 2] = blue;
        data[offset + 3] = 255;
        checksum = (checksum + red + green + blue) & 65535;
    }
}

checksum + data[0] + data[size - 1]
