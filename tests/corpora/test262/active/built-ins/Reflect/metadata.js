// Reflect namespace shape: it is a non-callable ordinary object exposing the
// standard static methods with the correct name and length metadata, plus a
// non-writable, non-enumerable, configurable Symbol.toStringTag of "Reflect".

assert.sameValue(typeof Reflect, "object", "Reflect is an object");
assert.sameValue(
  Object.prototype.toString.call(Reflect),
  "[object Reflect]",
  "Reflect Symbol.toStringTag"
);

var methods = [
  ["apply", 3],
  ["construct", 2],
  ["defineProperty", 3],
  ["deleteProperty", 2],
  ["get", 2],
  ["getOwnPropertyDescriptor", 2],
  ["getPrototypeOf", 1],
  ["has", 2],
  ["isExtensible", 1],
  ["ownKeys", 1],
  ["preventExtensions", 1],
  ["set", 3],
  ["setPrototypeOf", 2]
];

for (var i = 0; i < methods.length; i++) {
  var name = methods[i][0];
  var length = methods[i][1];
  var fn = Reflect[name];
  assert.sameValue(typeof fn, "function", "Reflect." + name + " is callable");
  assert.sameValue(fn.name, name, "Reflect." + name + ".name");
  assert.sameValue(fn.length, length, "Reflect." + name + ".length");
}

var tagDescriptor = Object.getOwnPropertyDescriptor(
  Reflect,
  Symbol.toStringTag
);
assert.sameValue(tagDescriptor.value, "Reflect", "toStringTag value");
assert.sameValue(tagDescriptor.writable, false, "toStringTag writable");
assert.sameValue(tagDescriptor.enumerable, false, "toStringTag enumerable");
assert.sameValue(tagDescriptor.configurable, true, "toStringTag configurable");

// Reflect itself is not a constructor and not callable.
assert.throws(TypeError, function () {
  Reflect();
});

42
