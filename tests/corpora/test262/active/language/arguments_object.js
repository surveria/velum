function probe(a, b) {
  return arguments.length + ":" + arguments[0] + ":" + arguments[2];
}
if (probe(10, 20, 30) !== "3:10:30") {
  throw new Test262Error("indexed arguments mismatch");
}

function count() {
  return arguments.length;
}
if (count() !== 0 || count(1) !== 1 || count(1, 2, 3) !== 3) {
  throw new Test262Error("arguments length mismatch");
}

function unmapped(a) {
  arguments[0] = 99;
  return a + ":" + arguments[0];
}
if (unmapped(1) !== "1:99") {
  throw new Test262Error("arguments writes must not alias parameters");
}

function byParam(arguments) {
  return arguments;
}
if (byParam("param") !== "param") {
  throw new Test262Error("parameter named arguments must shadow the object");
}

function total() {
  var sum = 0;
  for (var i = 0; i < arguments.length; i = i + 1) {
    sum = sum + arguments[i];
  }
  return sum;
}
if (total(1, 2, 3, 4) !== 10) {
  throw new Test262Error("arguments loop mismatch");
}

function spreadThrough() {
  return Math.max(...arguments);
}
if (spreadThrough(3, 9, 4) !== 9) {
  throw new Test262Error("arguments spread mismatch");
}

42;
