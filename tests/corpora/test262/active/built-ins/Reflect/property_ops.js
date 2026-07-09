// Reflect property operations: get, set, has, deleteProperty, defineProperty,
// getOwnPropertyDescriptor and ownKeys over ordinary objects, including getter
// dispatch, accessor definition and boolean success results.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

var target = { alpha: 1 };
Object.defineProperty(target, "beta", {
  value: 2,
  enumerable: false,
  writable: true,
  configurable: true
});

if (
  Reflect.get(target, "alpha") !== 1 ||
  Reflect.get(target, "beta") !== 2 ||
  Reflect.get(target, "missing") !== undefined ||
  Reflect.has(target, "alpha") !== true ||
  Reflect.has(target, "beta") !== true ||
  Reflect.has(target, "toString") !== true ||
  Reflect.has(target, "missing") !== false
) {
  throw new Test262Error("Reflect get/has mismatch");
}

if (
  Reflect.set(target, "alpha", 10) !== true ||
  target.alpha !== 10 ||
  Reflect.set(target, "gamma", 3) !== true ||
  target.gamma !== 3
) {
  throw new Test262Error("Reflect.set mismatch");
}

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
if (Reflect.get(withGetter, "computed") !== 41 || accessed !== 1) {
  throw new Test262Error("Reflect.get getter dispatch mismatch");
}

// defineProperty returns a boolean and installs the descriptor.
var defined = Reflect.defineProperty(target, "delta", {
  value: 4,
  enumerable: true,
  writable: false,
  configurable: false
});
var deltaDescriptor = Reflect.getOwnPropertyDescriptor(target, "delta");
if (
  defined !== true ||
  deltaDescriptor.value !== 4 ||
  deltaDescriptor.writable !== false ||
  deltaDescriptor.enumerable !== true ||
  deltaDescriptor.configurable !== false ||
  Reflect.getOwnPropertyDescriptor(target, "missing") !== undefined
) {
  throw new Test262Error("Reflect.defineProperty/getOwnPropertyDescriptor mismatch");
}

// deleteProperty removes configurable properties and reports success.
if (
  Reflect.deleteProperty(target, "gamma") !== true ||
  Reflect.has(target, "gamma") !== false
) {
  throw new Test262Error("Reflect.deleteProperty mismatch");
}

// ownKeys returns the string-keyed own properties in insertion order.
var keysTarget = { one: 1, two: 2 };
keysTarget.three = 3;
Object.defineProperty(keysTarget, "hidden", {
  value: 4,
  enumerable: false,
  configurable: true
});
var symbol = Symbol("reflect-key");
var registered = Symbol.for("shared-reflect-key");
keysTarget[symbol] = 5;
keysTarget[registered] = 6;
var keys = Reflect.ownKeys(keysTarget);
if (
  keys.length !== 6 ||
  keys[0] !== "one" ||
  keys[1] !== "two" ||
  keys[2] !== "three" ||
  keys[3] !== "hidden" ||
  keys[4] !== symbol ||
  keys[5] !== registered ||
  Symbol.for("shared-reflect-key") !== registered ||
  Symbol.keyFor(registered) !== "shared-reflect-key"
) {
  throw new Test262Error("Reflect.ownKeys mismatch");
}

// Non-object targets raise TypeError for property operations.
function throwsType(thunk) {
  try {
    thunk();
    return false;
  } catch (error) {
    return error instanceof TypeError;
  }
}

if (
  !throwsType(function () { return Reflect.get(42, "x"); }) ||
  !throwsType(function () { return Reflect.set("string", "x", 1); }) ||
  !throwsType(function () { return Reflect.has(null, "x"); }) ||
  !throwsType(function () { return Reflect.ownKeys(undefined); })
) {
  throw new Test262Error("Reflect property operations should reject non-object targets");
}

var proxyHasThrow = false;
try {
  Reflect.has(new Proxy({}, {
    has: function () {
      throw new TypeError("proxy has trap");
    }
  }), "x");
} catch (error) {
  proxyHasThrow = error instanceof TypeError;
}
if (proxyHasThrow !== true) {
  throw new Test262Error("Reflect.has should preserve proxy trap throws");
}

42
