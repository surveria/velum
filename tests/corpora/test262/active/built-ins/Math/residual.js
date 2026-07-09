let piDescriptor = Object.getOwnPropertyDescriptor(Math, "PI");
let tagDescriptor = Object.getOwnPropertyDescriptor(Math, Symbol.toStringTag);
let globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "Math");
let f16Descriptor = Object.getOwnPropertyDescriptor(Math, "f16round");
let sumDescriptor = Object.getOwnPropertyDescriptor(Math, "sumPrecise");

let f16Ok =
  Math.f16round(1.1) === 1.099609375 &&
  Math.f16round(2049) === 2048 &&
  Math.f16round(2051) === 2052 &&
  Math.f16round(65520) === Infinity &&
  Math.f16round(65519.99999999999) === 65504 &&
  Math.f16round(2.9802322387695312e-8) === 0 &&
  Math.f16round(2.980232238769532e-8) === 5.960464477539063e-8 &&
  1 / Math.f16round(-0) === -Infinity;

let roundOk =
  1 / Math.round(-0.5) === -Infinity &&
  1 / Math.round(0.5 - Number.EPSILON / 4) === Infinity &&
  Math.round(2 / Number.EPSILON - 1) === 2 / Number.EPSILON - 1;

let coercionCalls = 0;
let coerceToNaN = {
  valueOf: function () {
    coercionCalls++;
  }
};
let hypotCoercions = 0;
let coerceHypot = {
  valueOf: function () {
    hypotCoercions++;
    return 3;
  }
};

let coercionOk =
  Math.max(NaN, coerceToNaN) !== Math.max(NaN, coerceToNaN) &&
  Math.min(NaN, coerceToNaN) !== Math.min(NaN, coerceToNaN) &&
  Math.hypot(Infinity, coerceHypot, NaN) === Infinity &&
  coercionCalls === 4 &&
  hypotCoercions === 1;

let powOk =
  Math.pow(-Infinity, NaN) !== Math.pow(-Infinity, NaN) &&
  Math.pow(1, Infinity) !== Math.pow(1, Infinity) &&
  Math.pow(-1, -Infinity) !== Math.pow(-1, -Infinity);

let sumOk =
  Object.is(Math.sumPrecise([]), -0) &&
  Object.is(Math.sumPrecise([-0]), -0) &&
  Math.sumPrecise([-0, 0]) === 0 &&
  Math.sumPrecise([1, 2, 3]) === 6 &&
  Math.sumPrecise([1e30, 0.1, -1e30]) === 0.1 &&
  Math.sumPrecise([Infinity, -Infinity]) !== Math.sumPrecise([Infinity, -Infinity]) &&
  Math.sumPrecise([Infinity]) === Infinity;

function isConstructor(value) {
  try {
    Reflect.construct(function () {}, [], value);
  } catch (error) {
    return false;
  }
  return true;
}

let nonConstructorOk =
  !isConstructor(Math.abs) &&
  !isConstructor(Math.max) &&
  !isConstructor(Math.min) &&
  !isConstructor(Math.pow) &&
  !isConstructor(Math.random) &&
  !isConstructor(Math.sumPrecise);

let descriptorOk =
  piDescriptor.value === Math.PI &&
  piDescriptor.writable === false &&
  piDescriptor.enumerable === false &&
  piDescriptor.configurable === false &&
  Math[Symbol.toStringTag] === "Math" &&
  tagDescriptor.writable === false &&
  tagDescriptor.enumerable === false &&
  tagDescriptor.configurable === true &&
  globalDescriptor.value === Math &&
  globalDescriptor.writable === true &&
  globalDescriptor.enumerable === false &&
  globalDescriptor.configurable === true &&
  Math.f16round.name === "f16round" &&
  Math.f16round.length === 1 &&
  f16Descriptor.writable === true &&
  f16Descriptor.enumerable === false &&
  f16Descriptor.configurable === true &&
  Math.sumPrecise.name === "sumPrecise" &&
  Math.sumPrecise.length === 1 &&
  sumDescriptor.writable === true &&
  sumDescriptor.enumerable === false &&
  sumDescriptor.configurable === true;

if (
  !descriptorOk ||
  !f16Ok ||
  !roundOk ||
  !coercionOk ||
  !powOk ||
  !sumOk ||
  !nonConstructorOk
) {
  throw new Test262Error("Math residual behavior was unexpected");
}

42
