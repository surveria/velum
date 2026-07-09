// Reflect property operations: get, set, has, deleteProperty, defineProperty,
// getOwnPropertyDescriptor and ownKeys over ordinary objects, including getter
// dispatch, accessor definition and boolean success results.

var target = { alpha: 1 };
Object.defineProperty(target, "beta", {
  value: 2,
  enumerable: false,
  writable: true,
  configurable: true
});

assert.sameValue(Reflect.get(target, "alpha"), 1, "get data property");
assert.sameValue(Reflect.get(target, "beta"), 2, "get non-enumerable property");
assert.sameValue(Reflect.get(target, "missing"), undefined, "get absent property");

assert.sameValue(Reflect.has(target, "alpha"), true, "has own property");
assert.sameValue(Reflect.has(target, "beta"), true, "has hidden property");
assert.sameValue(Reflect.has(target, "toString"), true, "has inherited property");
assert.sameValue(Reflect.has(target, "missing"), false, "has absent property");

assert.sameValue(Reflect.set(target, "alpha", 10), true, "set returns true");
assert.sameValue(target.alpha, 10, "set mutates the property");
assert.sameValue(Reflect.set(target, "gamma", 3), true, "set new property");
assert.sameValue(target.gamma, 3, "set creates the property");

// Getter dispatch through Reflect.get.
var accessed = 0;
var withGetter = {};
Object.defineProperty(withGetter, "computed", {
  get: function () {
    accessed++;
    return 41;
  },
  configurable: true
});
assert.sameValue(Reflect.get(withGetter, "computed"), 41, "get invokes getter");
assert.sameValue(accessed, 1, "getter invoked exactly once");

// defineProperty returns a boolean and installs the descriptor.
var defined = Reflect.defineProperty(target, "delta", {
  value: 4,
  enumerable: true,
  writable: false,
  configurable: false
});
assert.sameValue(defined, true, "defineProperty returns true");
var deltaDescriptor = Reflect.getOwnPropertyDescriptor(target, "delta");
assert.sameValue(deltaDescriptor.value, 4, "descriptor value");
assert.sameValue(deltaDescriptor.writable, false, "descriptor writable");
assert.sameValue(deltaDescriptor.enumerable, true, "descriptor enumerable");
assert.sameValue(deltaDescriptor.configurable, false, "descriptor configurable");
assert.sameValue(
  Reflect.getOwnPropertyDescriptor(target, "missing"),
  undefined,
  "descriptor for absent property is undefined"
);

// deleteProperty removes configurable properties and reports success.
assert.sameValue(Reflect.deleteProperty(target, "gamma"), true, "delete configurable");
assert.sameValue(Reflect.has(target, "gamma"), false, "property removed");

// ownKeys returns the string-keyed own properties.
var keysTarget = { one: 1, two: 2 };
keysTarget.three = 3;
var keys = Reflect.ownKeys(keysTarget);
assert.sameValue(keys.length, 3, "ownKeys length");
assert.sameValue(keys[0], "one", "ownKeys order 0");
assert.sameValue(keys[1], "two", "ownKeys order 1");
assert.sameValue(keys[2], "three", "ownKeys order 2");

// Non-object targets raise TypeError for property operations.
assert.throws(TypeError, function () {
  Reflect.get(42, "x");
});
assert.throws(TypeError, function () {
  Reflect.set("string", "x", 1);
});
assert.throws(TypeError, function () {
  Reflect.has(null, "x");
});
assert.throws(TypeError, function () {
  Reflect.ownKeys(undefined);
});

42
