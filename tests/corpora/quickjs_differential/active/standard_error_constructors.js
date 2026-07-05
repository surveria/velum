var value = 0;
var plain = Error("plain", value = value + 1);
var typed = new TypeError("typed");
var syntax = SyntaxError("syntax");
var caught = "none";

try {
  throw new RangeError("range");
} catch (error) {
  caught = error.name + ":" + error.message;
}

print(plain.name, plain.message, typed.name, typed.message, syntax.name, syntax.message);
print(TypeError.name, TypeError.length, TypeError.prototype.constructor === TypeError);
print(caught, value);
