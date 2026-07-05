let f = function namedCamera(a, b) {};
let initialName = Object.getOwnPropertyDescriptor(f, "name");
let initialLength = Object.getOwnPropertyDescriptor(f, "length");

let deletedName = delete f.name;
let hasNameAfterDelete = Object.hasOwn(f, "name");

Object.defineProperty(f, "name", {
  value: "patched",
  writable: true,
  enumerable: true,
  configurable: true
});
f.name = "assigned";
let keysAfterName = Object.keys(f);

Object.defineProperty(f, "length", {
  value: 5,
  writable: true,
  configurable: true
});
f.length = 7;
let deletedLength = delete f.length;
let hasLengthAfterDelete = Object.hasOwn(f, "length");
Object.defineProperty(f, "length", {
  value: 11,
  writable: true,
  enumerable: true,
  configurable: true
});
let keysAfterLength = Object.keys(f);

Object.defineProperty(TypeError, "name", {
  value: "Typed",
  writable: true,
  configurable: true
});
TypeError.name = "TypedAssigned";
let assignedNativeName = TypeError.name;
let deletedNativeName = delete TypeError.name;
let nativeHasNameAfterDelete = Object.hasOwn(TypeError, "name");

Object.defineProperty(TypeError, "length", {
  value: 4,
  writable: true,
  configurable: true
});
TypeError.length = 6;
let assignedNativeLength = TypeError.length;
let deletedNativeLength = delete TypeError.length;
let nativeHasLengthAfterDelete = Object.hasOwn(TypeError, "length");

if (
  initialName.value !== "namedCamera" ||
  initialName.configurable !== true ||
  initialLength.value !== 2 ||
  initialLength.configurable !== true ||
  deletedName !== true ||
  hasNameAfterDelete !== false ||
  f.name !== "assigned" ||
  keysAfterName.length !== 1 ||
  keysAfterName[0] !== "name" ||
  f.length !== 11 ||
  deletedLength !== true ||
  hasLengthAfterDelete !== false ||
  keysAfterLength.length !== 2 ||
  keysAfterLength[1] !== "length" ||
  assignedNativeName !== "TypedAssigned" ||
  deletedNativeName !== true ||
  nativeHasNameAfterDelete !== false ||
  assignedNativeLength !== 6 ||
  deletedNativeLength !== true ||
  nativeHasLengthAfterDelete !== false
) {
  throw new Test262Error("Function intrinsic descriptor behavior was unexpected");
}

42
