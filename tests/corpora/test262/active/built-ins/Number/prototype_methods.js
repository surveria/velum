let boxed = new Number("255");
let objectBox = Object(15);

if (
  Number.prototype.valueOf.call(7) !== 7 ||
  boxed.valueOf() !== 255 ||
  objectBox.valueOf() !== 15 ||
  boxed.toString() !== "255" ||
  boxed.toString(16) !== "ff" ||
  objectBox.toString(2) !== "1111" ||
  Number.prototype.toLocaleString.call(42) !== "42" ||
  Number.isInteger(42) !== true ||
  Number.isInteger(42.5) !== false ||
  Number.isInteger("42") !== false ||
  Number.isSafeInteger(9007199254740991) !== true ||
  Number.isSafeInteger(9007199254740992) !== false
) {
  throw new Test262Error("Number prototype method behavior was unexpected");
}

let rejected = false;
try {
  Number.prototype.valueOf.call(true);
} catch (error) {
  rejected = error instanceof TypeError;
}

if (!rejected) {
  throw new Test262Error("Number.prototype.valueOf accepted a non-number receiver");
}

42
