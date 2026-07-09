// Reflect prototype inspection and extensibility control:
// getPrototypeOf, setPrototypeOf, isExtensible and preventExtensions.

var proto = { marker: 7 };
var object = Object.create(proto);

assert.sameValue(
  Reflect.getPrototypeOf(object),
  proto,
  "getPrototypeOf returns the installed prototype"
);
assert.sameValue(
  Reflect.getPrototypeOf({}),
  Object.prototype,
  "getPrototypeOf returns Object.prototype for plain objects"
);
assert.sameValue(
  Reflect.getPrototypeOf(Object.create(null)),
  null,
  "getPrototypeOf returns null for null-prototype objects"
);

var replacement = { swapped: true };
assert.sameValue(
  Reflect.setPrototypeOf(object, replacement),
  true,
  "setPrototypeOf returns true"
);
assert.sameValue(
  Reflect.getPrototypeOf(object),
  replacement,
  "setPrototypeOf installs the new prototype"
);
assert.sameValue(object.swapped, true, "inherited property visible after swap");
assert.sameValue(
  Reflect.setPrototypeOf(object, null),
  true,
  "setPrototypeOf accepts null"
);
assert.sameValue(
  Reflect.getPrototypeOf(object),
  null,
  "setPrototypeOf can clear the prototype"
);

var extensible = {};
assert.sameValue(Reflect.isExtensible(extensible), true, "new object is extensible");
assert.sameValue(
  Reflect.preventExtensions(extensible),
  true,
  "preventExtensions returns true"
);
assert.sameValue(
  Reflect.isExtensible(extensible),
  false,
  "object is no longer extensible"
);

// Non-object targets raise TypeError.
assert.throws(TypeError, function () {
  Reflect.getPrototypeOf(1);
});
assert.throws(TypeError, function () {
  Reflect.setPrototypeOf("x", null);
});
assert.throws(TypeError, function () {
  Reflect.isExtensible(true);
});
assert.throws(TypeError, function () {
  Reflect.preventExtensions(undefined);
});

42
