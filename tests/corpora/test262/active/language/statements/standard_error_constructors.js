let value = 0;
let plain = Error("plain", value = value + 1);
let typed = new TypeError("typed");
let syntax = SyntaxError("syntax");

assert.throws(TypeError, function() {
  throw new TypeError("boom");
}, "TypeError should match");

assert.throws(Error, function() {
  throw new RangeError("range");
});

if (plain.name !== "Error") {
  throw new Test262Error("Error name mismatch");
}
if (plain.message !== "plain") {
  throw new Test262Error("Error message mismatch");
}
if (typed.name !== "TypeError") {
  throw new Test262Error("TypeError name mismatch");
}
if (typed.message !== "typed") {
  throw new Test262Error("TypeError message mismatch");
}
if (syntax.name !== "SyntaxError") {
  throw new Test262Error("SyntaxError name mismatch");
}
if (syntax.message !== "syntax") {
  throw new Test262Error("SyntaxError message mismatch");
}
if (TypeError.name !== "TypeError") {
  throw new Test262Error("TypeError constructor name mismatch");
}
if (TypeError.length !== 1) {
  throw new Test262Error("TypeError constructor length mismatch");
}
if (TypeError.prototype.constructor !== TypeError) {
  throw new Test262Error("TypeError prototype constructor mismatch");
}

value = value + 41;
value;
