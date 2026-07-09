// Callable Proxy apply and construct traps, plus target fallback.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

function add(a, b) {
  return a + b;
}

var log = [];
var applied = new Proxy(add, {
  apply: function (target, thisArg, argsList) {
    log.push("apply:" + argsList.length);
    return target.apply(thisArg, argsList) * 10;
  }
});

if (applied(2, 3) !== 50) {
  throw new Test262Error("proxy apply trap mismatch");
}

function Point(x, y) {
  this.x = x;
  this.y = y;
}
Point.prototype.sum = function () {
  return this.x + this.y;
};

var constructed = new Proxy(Point, {
  construct: function (target, argsList) {
    log.push("construct:" + argsList.length);
    return new target(argsList[0], argsList[1]);
  }
});

var point = new constructed(3, 4);
if (point.x !== 3 || point.y !== 4 || point.sum() !== 7) {
  throw new Test262Error("proxy construct trap mismatch");
}

if (log.join(",") !== "apply:2,construct:2") {
  throw new Test262Error("proxy callable trap order mismatch: " + log.join(","));
}

// No-trap fallback calls/constructs the target directly.
var plainCall = new Proxy(add, {});
if (plainCall(4, 5) !== 9) {
  throw new Test262Error("proxy apply fallback mismatch");
}
var plainConstruct = new Proxy(Point, {});
var fallbackPoint = new plainConstruct(5, 6);
if (fallbackPoint.x !== 5 || fallbackPoint.sum() !== 11) {
  throw new Test262Error("proxy construct fallback mismatch");
}

42
