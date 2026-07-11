let value = 0;
let plain = Error("plain", value = value + 1);
let typed = new TypeError("typed");
let syntax = SyntaxError("syntax");

assert.throws(TypeError, function() {
  throw new TypeError("boom");
}, "TypeError should match");

assert.throws(RangeError, function() {
  throw new RangeError("range");
});

if (plain.name === "Error" && plain.message === "plain") {
  value = value + 10;
}
if (typed.name === "TypeError" && typed.message === "typed") {
  value = value + 10;
}
if (syntax.name === "SyntaxError" && syntax.message === "syntax") {
  value = value + 10;
}
if (TypeError.name === "TypeError" && TypeError.length === 1) {
  value = value + 5;
}
if (TypeError.prototype.constructor === TypeError) {
  value = value + 6;
}

print(plain.name, plain.message, typed.name, typed.message, syntax.name, syntax.message);
value;
