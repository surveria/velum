// Reflect.apply and Reflect.construct: argument-list spreading through
// array-like objects, this binding, constructor invocation and error paths.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

function collect(a, b, c) {
  return [this.tag, a, b, c].join(",");
}

var receiver = { tag: "ctx" };
var arrayLike = { length: 2, 0: "x", 1: "y" };

function sum() {
  var total = 0;
  for (var i = 0; i < arguments.length; i++) {
    total += arguments[i];
  }
  return total;
}

if (
  Reflect.apply(collect, receiver, [1, 2, 3]) !== "ctx,1,2,3" ||
  Reflect.apply(collect, receiver, arrayLike) !== "ctx,x,y," ||
  Reflect.apply(sum, null, [4, 5, 6]) !== 15
) {
  throw new Test262Error("Reflect.apply mismatch");
}

// construct creates an instance whose prototype chain and fields are correct.
function Point(x, y) {
  this.x = x;
  this.y = y;
}
Point.prototype.norm = function () {
  return this.x + this.y;
};

var point = Reflect.construct(Point, [3, 4]);
if (
  point.x !== 3 ||
  point.y !== 4 ||
  !(point instanceof Point) ||
  point.norm() !== 7
) {
  throw new Test262Error("Reflect.construct mismatch");
}

// Error paths: non-callable apply target and non-constructor construct target.
function throwsType(thunk) {
  try {
    thunk();
    return false;
  } catch (error) {
    return error instanceof TypeError;
  }
}

if (
  !throwsType(function () { return Reflect.apply({}, null, []); }) ||
  !throwsType(function () { return Reflect.construct({}, []); }) ||
  !throwsType(function () { return Reflect.apply(collect, receiver, 5); })
) {
  throw new Test262Error("Reflect.apply/construct should reject invalid targets");
}

42
