var key = {};
var symbolKey = Symbol("weak");
var wm = new WeakMap();
wm.set(key, "object").set(symbolKey, "symbol");
if (wm.get(key) !== "object" || wm.get(symbolKey) !== "symbol") {
  throw new Test262Error("WeakMap object or symbol key mismatch");
}
if (!wm.has(key) || wm.has({})) {
  throw new Test262Error("WeakMap has mismatch");
}
if (!wm.delete(key) || wm.has(key) || wm.delete(key)) {
  throw new Test262Error("WeakMap delete mismatch");
}
if (wm.get(1) !== undefined || wm.has(1) || wm.delete(1)) {
  throw new Test262Error("WeakMap primitive lookup mismatch");
}

var seededKey = {};
if (new WeakMap([[seededKey, 9]]).get(seededKey) !== 9) {
  throw new Test262Error("WeakMap iterable seeding mismatch");
}

function errorKind(callback) {
  try {
    callback();
    return "none";
  } catch (error) {
    return error instanceof TypeError ? "TypeError" : "other";
  }
}

if (errorKind(function () { WeakMap(); }) !== "TypeError") {
  throw new Test262Error("WeakMap call without new must throw TypeError");
}
if (errorKind(function () { wm.set(1, 2); }) !== "TypeError") {
  throw new Test262Error("WeakMap set primitive key must throw TypeError");
}
if (errorKind(function () { WeakMap.prototype.get.call({}, key); }) !== "TypeError") {
  throw new Test262Error("WeakMap method on WeakSet receiver must throw TypeError");
}
if (!(new WeakMap() instanceof WeakMap)) {
  throw new Test262Error("WeakMap instanceof mismatch");
}
if (WeakMap.prototype.size !== undefined || WeakMap.prototype.clear !== undefined) {
  throw new Test262Error("WeakMap exposed strong collection properties");
}

42;
