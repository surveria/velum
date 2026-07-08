let falseBox = new Boolean(false);
let trueBox = new Boolean(1);
let objectBox = Object(false);

if (
  Boolean.prototype.valueOf.call(true) !== true ||
  falseBox.valueOf() !== false ||
  trueBox.valueOf() !== true ||
  objectBox.valueOf() !== false ||
  falseBox.toString() !== "false" ||
  trueBox.toString() !== "true" ||
  objectBox.toString() !== "false"
) {
  throw new Test262Error("Boolean prototype method behavior was unexpected");
}

let rejected = false;
try {
  Boolean.prototype.toString.call(0);
} catch (error) {
  rejected = error instanceof TypeError;
}

if (!rejected) {
  throw new Test262Error("Boolean.prototype.toString accepted a non-boolean receiver");
}

42
