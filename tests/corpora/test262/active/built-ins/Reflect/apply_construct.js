// Reflect.apply and Reflect.construct: argument-list spreading through
// array-like objects, this binding, constructor invocation and error paths.

function collect(a, b, c) {
  return [this.tag, a, b, c].join(",");
}

var receiver = { tag: "ctx" };
assert.sameValue(
  Reflect.apply(collect, receiver, [1, 2, 3]),
  "ctx,1,2,3",
  "apply binds this and spreads arguments"
);

// Array-like (non-Array) argument lists are accepted.
var arrayLike = { length: 2, 0: "x", 1: "y" };
assert.sameValue(
  Reflect.apply(collect, receiver, arrayLike),
  "ctx,x,y,",
  "apply accepts array-like argument lists"
);

function sum() {
  var total = 0;
  for (var i = 0; i < arguments.length; i++) {
    total += arguments[i];
  }
  return total;
}
assert.sameValue(Reflect.apply(sum, null, [4, 5, 6]), 15, "apply forwards all arguments");

// construct creates an instance whose prototype chain and fields are correct.
function Point(x, y) {
  this.x = x;
  this.y = y;
}
Point.prototype.norm = function () {
  return this.x + this.y;
};

var point = Reflect.construct(Point, [3, 4]);
assert.sameValue(point.x, 3, "construct sets first field");
assert.sameValue(point.y, 4, "construct sets second field");
assert.sameValue(point instanceof Point, true, "construct result is an instance");
assert.sameValue(point.norm(), 7, "construct wires the prototype");

// Error paths: non-callable apply target and non-constructor construct target.
assert.throws(TypeError, function () {
  Reflect.apply({}, null, []);
});
assert.throws(TypeError, function () {
  Reflect.construct({}, []);
});
assert.throws(TypeError, function () {
  Reflect.apply(collect, receiver, 5);
});

42
