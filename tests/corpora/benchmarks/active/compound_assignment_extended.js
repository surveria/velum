let value = 1;
let record = { mask: 3 };
let values = [1, 2, 3, 4];

for (let index = 0; index < 128; index++) {
    value |= index;
    value ^= index & 7;
    value <<= 1;
    value >>>= 1;
    record.mask ^= index;
    record.mask |= values[index & 3];
    values[index & 3] <<= 1;
    values[index & 3] >>= 1;
}

value + record.mask + values[0] + values[1] + values[2] + values[3]
