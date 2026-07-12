let buffer = new SharedArrayBuffer(24, { maxByteLength: 32 });
let ints = new Int32Array(buffer, 0, 2);
let big = new BigInt64Array(buffer, 8, 2);

let oldNumber = Atomics.store(ints, 0, 4);
let exchangedNumber = Atomics.add(ints, 0, 3);
let oldBigInt = Atomics.store(big, 0, -2n);
let exchangedBigInt = Atomics.compareExchange(big, 0, -2n, 9n);

buffer.grow(32);

oldNumber === 4 && exchangedNumber === 4 && Atomics.load(ints, 0) === 7 &&
    oldBigInt === -2n && exchangedBigInt === -2n && Atomics.load(big, 0) === 9n &&
    buffer.byteLength === 32 && buffer.growable ? 42 : 0;
