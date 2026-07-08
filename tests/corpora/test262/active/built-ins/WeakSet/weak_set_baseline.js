var setKey = {};
var weakSetSymbol = Symbol("set");
var ws = new WeakSet();
ws.add(setKey).add(weakSetSymbol);
if (!ws.has(setKey) || !ws.has(weakSetSymbol) || ws.has({})) {
  throw new Test262Error("WeakSet has mismatch");
}
if (!ws.delete(setKey) || ws.has(setKey) || ws.delete(setKey)) {
  throw new Test262Error("WeakSet delete mismatch");
}
if (ws.has("x") || ws.delete("x")) {
  throw new Test262Error("WeakSet primitive lookup mismatch");
}

var seededSetKey = {};
if (!new WeakSet([seededSetKey]).has(seededSetKey)) {
  throw new Test262Error("WeakSet iterable seeding mismatch");
}

function errorKind(callback) {
  try {
    callback();
    return "none";
  } catch (error) {
    return error instanceof TypeError ? "TypeError" : "other";
  }
}

if (errorKind(function () { WeakSet(); }) !== "TypeError") {
  throw new Test262Error("WeakSet call without new must throw TypeError");
}
if (errorKind(function () { ws.add(1); }) !== "TypeError") {
  throw new Test262Error("WeakSet add primitive key must throw TypeError");
}
if (errorKind(function () { WeakSet.prototype.has.call({}, setKey); }) !== "TypeError") {
  throw new Test262Error("WeakSet method on incompatible receiver must throw TypeError");
}
if (!(new WeakSet() instanceof WeakSet)) {
  throw new Test262Error("WeakSet instanceof mismatch");
}
if (WeakSet.prototype.size !== undefined || WeakSet.prototype.clear !== undefined) {
  throw new Test262Error("WeakSet exposed strong collection properties");
}

42;
