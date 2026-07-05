let f = function namedCamera(a, b) {};
Object.defineProperty(f, "tag", {
  value: "camera",
  enumerable: true,
  writable: false,
  configurable: false
});
Object.defineProperty(f, "hidden", { value: 9 });
f.tag = "changed";
let deleteTag = delete f.tag;
let tagDescriptor = Object.getOwnPropertyDescriptor(f, "tag");
let hiddenDescriptor = Object.getOwnPropertyDescriptor(f, "hidden");
let nameDescriptor = Object.getOwnPropertyDescriptor(f, "name");
let lengthDescriptor = Object.getOwnPropertyDescriptor(f, "length");
let functionKeys = Object.keys(f);

Object.defineProperty(Object.keys, "tag", {
  value: "native",
  enumerable: true,
  writable: false,
  configurable: false
});
Object.keys.tag = "changed";
let deleteNativeTag = delete Object.keys.tag;
let nativeTagDescriptor = Object.getOwnPropertyDescriptor(Object.keys, "tag");
let nativeNameDescriptor = Object.getOwnPropertyDescriptor(Object.keys, "name");
let nativeLengthDescriptor = Object.getOwnPropertyDescriptor(Object.keys, "length");
let nativeKeys = Object.keys(Object.keys);

if (
  f.tag !== "camera" ||
  functionKeys.length !== 1 ||
  functionKeys[0] !== "tag" ||
  tagDescriptor.enumerable !== true ||
  tagDescriptor.writable !== false ||
  tagDescriptor.configurable !== false ||
  deleteTag !== false ||
  hiddenDescriptor.value !== 9 ||
  hiddenDescriptor.enumerable !== false ||
  nameDescriptor.value !== "namedCamera" ||
  nameDescriptor.configurable !== true ||
  lengthDescriptor.value !== 2 ||
  lengthDescriptor.configurable !== true ||
  Object.keys.tag !== "native" ||
  nativeKeys.length !== 1 ||
  nativeKeys[0] !== "tag" ||
  nativeTagDescriptor.enumerable !== true ||
  nativeTagDescriptor.writable !== false ||
  nativeTagDescriptor.configurable !== false ||
  deleteNativeTag !== false ||
  nativeNameDescriptor.value !== "keys" ||
  nativeNameDescriptor.configurable !== true ||
  nativeLengthDescriptor.value !== 1 ||
  nativeLengthDescriptor.configurable !== true
) {
  throw new Test262Error("Function descriptor behavior was unexpected");
}

42
