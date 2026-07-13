const width = 128;
const height = 72;
const channels = 4;
const size = width * height * channels;
const rounds = 64;
const source = new Uint8Array(size);
const target = new Uint8Array(size);
let checksum = 0;

for (let round = 0; round < rounds; round += 1) {
    const value = (round * 17 + 29) & 255;
    source.fill(value);
    target.set(source);
    target.copyWithin(channels, 0, size - channels);
    target.reverse();
    checksum = (checksum + target[0] + target[size - 1]) & 65535;
}

checksum + source[width] + target[height]
