function tail(first, ...rest) {
  return first + "|" + rest.join(",");
}
if (tail(1, 2, 3) !== "1|2,3" || tail("only") !== "only|") {
  throw new Test262Error("rest parameter mismatch");
}

function pairRest(...[a, b]) {
  return "" + a + b;
}
if (pairRest(1, 2) !== "12") {
  throw new Test262Error("rest pattern parameter mismatch");
}

function lengthProbe(a, b, ...r) {}
if (lengthProbe.length !== 2) {
  throw new Test262Error("rest parameter must not count toward length");
}

function join4(a, b, c, d) {
  return "" + a + b + c + d;
}
if (join4(...[1, 2], 3, ...[4]) !== "1234") {
  throw new Test262Error("spread call arguments mismatch");
}

if (Math.max(...[3, 9, 4]) !== 9) {
  throw new Test262Error("spread native call mismatch");
}

var receiver = {
  base: 10,
  add: function (a, b) { return this.base + a + b; }
};
if (receiver.add(...[1, 2]) !== 13) {
  throw new Test262Error("spread method call mismatch");
}

function PairCtor(a, b) {
  this.sum = a + b;
}
if (new PairCtor(...[40, 2]).sum !== 42) {
  throw new Test262Error("spread construct mismatch");
}

var mixed = [0, ...[1, 2], ..."ab"];
if (mixed.join("|") !== "0|1|2|a|b") {
  throw new Test262Error("array spread mismatch");
}

var baseObject = { x: 1, y: 2 };
var merged = { w: 0, ...baseObject, y: 9, ...null, ...undefined };
if (merged.w !== 0 || merged.x !== 1 || merged.y !== 9 || baseObject.y !== 2) {
  throw new Test262Error("object spread mismatch");
}

var caught = "";
try {
  var broken = [...5];
} catch (error) {
  if (error instanceof TypeError) {
    caught = "TypeError";
  }
}
if (caught !== "TypeError") {
  throw new Test262Error("non-iterable spread must throw TypeError");
}

42;
