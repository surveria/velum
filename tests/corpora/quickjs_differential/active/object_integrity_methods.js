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

print(
  Object.preventExtensions.length,
  Object.isExtensible.length,
  Object.seal.length,
  Object.freeze.length,
  Object.isSealed.length,
  Object.isFrozen.length
);
print(
  before,
  prevented === object,
  Object.isExtensible(object),
  object.b,
  defineRejected,
  protoRejected
);
print(descriptor.value, descriptor.writable, descriptor.configurable);
print(
  sealedReturned === sealed,
  Object.isSealed(sealed),
  Object.isFrozen(sealed),
  sealed.a,
  deleteSealed
);
print(sealedDescriptor.writable, sealedDescriptor.configurable);
print(hiddenDescriptor.writable, hiddenDescriptor.configurable);
print(
  frozenReturned === frozen,
  Object.isSealed(frozen),
  Object.isFrozen(frozen),
  frozen.a,
  deleteFrozen
);
print(frozenDescriptor.writable, frozenDescriptor.configurable);
print(array[0], array[2], Object.isFrozen(array), element.writable, length.writable);
print(
  Object.preventExtensions(7),
  Object.seal("x"),
  Object.freeze(true),
  Object.preventExtensions(undefined),
  Object.freeze(null)
);
print(
  Object.isExtensible(7),
  Object.isExtensible(null),
  Object.isSealed("x"),
  Object.isSealed(undefined),
  Object.isFrozen(true),
  Object.isFrozen(null)
);

print(42);
