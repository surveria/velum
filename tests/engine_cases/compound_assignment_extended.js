let value = 5;
let orValue = value |= 2;
let xorValue = value ^= 3;
let left = value <<= 2;
let right = value >>= 1;
let unsigned = value >>>= 1;
let power = value **= 3;
print(orValue, xorValue, left, right, unsigned, power, value);

let negative = -8;
let signed = negative >>= 1;
let unsignedRight = negative >>>= 1;
print(signed, unsignedRight, negative);

let flags = { mask: 1 };
let propOr = flags.mask |= 4;
let propXor = flags.mask ^= 7;
let propShift = flags.mask <<= 3;
print(propOr, propXor, propShift, flags.mask);

let values = [1, 8];
let cellLeft = values[0] <<= 5;
let cellRight = values[1] >>= 2;
let cellPower = values[1] **= 5;
print(cellLeft, cellRight, cellPower, values[0], values[1]);

let order = "";
let target = { slot: 32 };
let key = function() {
    order += "k";
    return "slot";
};
let rhs = function() {
    order += "r";
    return 10;
};
let ordered = target[key()] ^= rhs();
print(order, ordered, target.slot);

let binary =
    (1 | 4) +
    (7 ^ 3) +
    (1 << 4) +
    (16 >> 2) +
    (-1 >>> 30) +
    (2 ** 3 ** 2);

orValue === 7 &&
    xorValue === 4 &&
    left === 16 &&
    right === 8 &&
    unsigned === 4 &&
    power === 64 &&
    value === 64 &&
    signed === -4 &&
    unsignedRight === 2147483646 &&
    negative === 2147483646 &&
    propOr === 5 &&
    propXor === 2 &&
    propShift === 16 &&
    flags.mask === 16 &&
    cellLeft === 32 &&
    cellRight === 2 &&
    cellPower === 32 &&
    values[0] === 32 &&
    values[1] === 32 &&
    order === "kr" &&
    ordered === 42 &&
    target.slot === 42 &&
    binary === 544 ? 42 : 0
