const width = 80;
const height = 45;
const channels = 4;
const size = width * height * channels;
const data = typeof __imageData === "undefined" ? new Uint8Array(size) : __imageData;
let checksum = 0;

for (let index = 0; index < size; index += channels) {
    data[index] = 32;
    data[index + 1] = 128;
    data[index + 2] = 224;
    data[index + 3] = 255;
    checksum = (checksum + data[index] + data[index + 1] + data[index + 2]) & 65535;
}

checksum + data[0] + data[size - 1]
