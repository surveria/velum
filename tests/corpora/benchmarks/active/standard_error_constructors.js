var i = 0;
var total = 0;

while (i < 2048) {
  var plain = Error("plain");
  if (plain.name === "Error") {
    total = total + 1;
  }

  var typed = new TypeError("typed");
  if (typed.message === "typed") {
    total = total + 1;
  }

  var syntax = SyntaxError("syntax");
  if (syntax.name === "SyntaxError") {
    total = total + 1;
  }

  try {
    throw new RangeError("range");
  } catch (error) {
    if (error.name === "RangeError") {
      total = total + 1;
    }
  }

  i = i + 1;
}

total;
