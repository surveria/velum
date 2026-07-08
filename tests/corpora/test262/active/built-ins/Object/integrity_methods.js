let object = { a: 1 };
let before = Object.isExtensible(object);
let prevented = Object.preventExtensions(object);
object.b = 2;

let defineRejected = false;
try {
  Object.defineProperty(object, "c", { value: 3 });
} catch (error) {
  defineRejected = error instanceof TypeError;
}

let protoRejected = false;
try {
  Object.setPrototypeOf(object, { p: 1 });
} catch (error) {
  protoRejected = error instanceof TypeError;
}

Object.defineProperty(object, "a", {
  value: 7,
  enumerable: true,
  writable: true,
  configurable: true
});

let descriptor = Object.getOwnPropertyDescriptor(object, "a");

let sealed = { a: 1 };
Object.defineProperty(sealed, "hidden", { value: 2 });
let sealedReturned = Object.seal(sealed);
sealed.a = 3;
let deleteSealed = delete sealed.a;
let sealedDescriptor = Object.getOwnPropertyDescriptor(sealed, "a");
let hiddenDescriptor = Object.getOwnPropertyDescriptor(sealed, "hidden");

let frozen = { a: 1 };
let frozenReturned = Object.freeze(frozen);
frozen.a = 5;
let deleteFrozen = delete frozen.a;
let frozenDescriptor = Object.getOwnPropertyDescriptor(frozen, "a");

let array = [1, 2];
Object.freeze(array);
array[0] = 9;
array[2] = 3;
let element = Object.getOwnPropertyDescriptor(array, "0");
let length = Object.getOwnPropertyDescriptor(array, "length");

if (
  Object.preventExtensions.length !== 1 ||
  Object.isExtensible.length !== 1 ||
  Object.seal.length !== 1 ||
  Object.freeze.length !== 1 ||
  Object.isSealed.length !== 1 ||
  Object.isFrozen.length !== 1 ||
  before !== true ||
  prevented !== object ||
  Object.isExtensible(object) !== false ||
  object.b !== undefined ||
  defineRejected !== true ||
  protoRejected !== true ||
  descriptor.value !== 7 ||
  descriptor.writable !== true ||
  descriptor.configurable !== true ||
  sealedReturned !== sealed ||
  Object.isSealed(sealed) !== true ||
  Object.isFrozen(sealed) !== false ||
  sealed.a !== 3 ||
  deleteSealed !== false ||
  sealedDescriptor.writable !== true ||
  sealedDescriptor.configurable !== false ||
  hiddenDescriptor.writable !== false ||
  hiddenDescriptor.configurable !== false ||
  frozenReturned !== frozen ||
  Object.isSealed(frozen) !== true ||
  Object.isFrozen(frozen) !== true ||
  frozen.a !== 1 ||
  deleteFrozen !== false ||
  frozenDescriptor.writable !== false ||
  frozenDescriptor.configurable !== false ||
  array[0] !== 1 ||
  array[2] !== undefined ||
  Object.isFrozen(array) !== true ||
  element.writable !== false ||
  length.writable !== false ||
  Object.preventExtensions(7) !== 7 ||
  Object.seal("x") !== "x" ||
  Object.freeze(true) !== true ||
  Object.preventExtensions(undefined) !== undefined ||
  Object.freeze(null) !== null ||
  Object.isExtensible(7) !== false ||
  Object.isExtensible(null) !== false ||
  Object.isSealed("x") !== true ||
  Object.isSealed(undefined) !== true ||
  Object.isFrozen(true) !== true ||
  Object.isFrozen(null) !== true
) {
  throw new Test262Error("Object integrity method behavior was unexpected");
}

42
