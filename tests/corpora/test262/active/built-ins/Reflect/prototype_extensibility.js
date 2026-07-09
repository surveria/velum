// Reflect prototype inspection and extensibility control:
// getPrototypeOf, setPrototypeOf, isExtensible and preventExtensions.
//
// Self-contained active fixture: evaluates to 42 on success, throws a
// Test262Error on failure, and produces no output.

var proto = { marker: 7 };
var object = Object.create(proto);

if (
  Reflect.getPrototypeOf(object) !== proto ||
  Reflect.getPrototypeOf({}) !== Object.prototype ||
  Reflect.getPrototypeOf(Object.create(null)) !== null
) {
  throw new Test262Error("Reflect.getPrototypeOf mismatch");
}

var replacement = { swapped: true };
if (
  Reflect.setPrototypeOf(object, replacement) !== true ||
  Reflect.getPrototypeOf(object) !== replacement ||
  object.swapped !== true ||
  Reflect.setPrototypeOf(object, null) !== true ||
  Reflect.getPrototypeOf(object) !== null
) {
  throw new Test262Error("Reflect.setPrototypeOf mismatch");
}

var extensible = {};
if (
  Reflect.isExtensible(extensible) !== true ||
  Reflect.preventExtensions(extensible) !== true ||
  Reflect.isExtensible(extensible) !== false
) {
  throw new Test262Error("Reflect extensibility control mismatch");
}

// Non-object targets raise TypeError.
function throwsType(thunk) {
  try {
    thunk();
    return false;
  } catch (error) {
    return error instanceof TypeError;
  }
}

if (
  !throwsType(function () { return Reflect.getPrototypeOf(1); }) ||
  !throwsType(function () { return Reflect.setPrototypeOf("x", null); }) ||
  !throwsType(function () { return Reflect.isExtensible(true); }) ||
  !throwsType(function () { return Reflect.preventExtensions(undefined); })
) {
  throw new Test262Error("Reflect prototype/extensibility should reject non-object targets");
}

42
