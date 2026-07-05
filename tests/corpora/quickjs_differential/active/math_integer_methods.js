let metadataOk =
    Math.clz32.name === "clz32" &&
    Math.clz32.length === 1 &&
    Math.fround.name === "fround" &&
    Math.fround.length === 1 &&
    Math.imul.name === "imul" &&
    Math.imul.length === 2;

let clzOk =
    Math.clz32(0) === 32 &&
    Math.clz32(-0) === 32 &&
    Math.clz32(1) === 31 &&
    Math.clz32(2147483648) === 0 &&
    Math.clz32(4294967296) === 32 &&
    Math.clz32(-4294967297) === 0 &&
    Math.clz32(NaN) === 32 &&
    Math.clz32(Infinity) === 32 &&
    Math.clz32("0x10") === 27;

let imulOk =
    Math.imul(2, 4) === 8 &&
    Math.imul(-1, 8) === -8 &&
    Math.imul(0xffffffff, 5) === -5 &&
    Math.imul(65535, 65535) === -131071 &&
    Math.imul(1.9, 7) === 7 &&
    Math.imul(7) === 0 &&
    Math.imul("0x10", 2) === 32;

let froundOk =
    Math.fround(0.1) === 0.10000000149011612 &&
    Math.fround(4294967295) === 4294967296 &&
    Math.fround(NaN) !== Math.fround(NaN) &&
    Math.fround(Infinity) === Infinity &&
    1 / Math.fround(-0) === -Infinity &&
    Math.fround("0.5") === 0.5;

let froundTieOk =
    Math.fround(1.0000000596046448) === 1 &&
    Math.fround(1.0000001788139343) === 1.000000238418579;

print(metadataOk, clzOk, imulOk);
print(froundOk, froundTieOk);
