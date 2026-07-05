let value = 5;
let orValue = value |= 2;
let xorValue = value ^= 3;
let left = value <<= 2;
let right = value >>= 1;
let unsigned = value >>>= 1;
let power = value **= 3;
if (
    orValue !== 7 ||
    xorValue !== 4 ||
    left !== 16 ||
    right !== 8 ||
    unsigned !== 4 ||
    power !== 64 ||
    value !== 64
) {
    throw new Test262Error("extended compound assignment did not update binding values");
}

let negative = -8;
let signed = negative >>= 1;
let unsignedRight = negative >>>= 1;
if (signed !== -4 || unsignedRight !== 2147483646 || negative !== 2147483646) {
    throw new Test262Error("shift compound assignment produced unexpected values");
}

let flags = { mask: 1 };
let propOr = flags.mask |= 4;
let propXor = flags.mask ^= 7;
let propShift = flags.mask <<= 3;
if (propOr !== 5 || propXor !== 2 || propShift !== 16 || flags.mask !== 16) {
    throw new Test262Error("extended property compound assignment did not store values");
}

let values = [1, 8];
let cellLeft = values[0] <<= 5;
let cellRight = values[1] >>= 2;
let cellPower = values[1] **= 5;
if (
    cellLeft !== 32 ||
    cellRight !== 2 ||
    cellPower !== 32 ||
    values[0] !== 32 ||
    values[1] !== 32
) {
    throw new Test262Error("extended computed compound assignment did not store values");
}

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
if (order !== "kr" || ordered !== 42 || target.slot !== 42) {
    throw new Test262Error("extended compound assignment used an unexpected evaluation order");
}

let binary =
    (1 | 4) +
    (7 ^ 3) +
    (1 << 4) +
    (16 >> 2) +
    (-1 >>> 30) +
    (2 ** 3 ** 2);
if (binary !== 544) {
    throw new Test262Error("extended binary operators produced an unexpected value");
}

42
