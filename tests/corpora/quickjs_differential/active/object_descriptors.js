let objectKeys = "";
for (let key in Object) {
    objectKeys = objectKeys + key + ";";
}

let object = { a: 1 };
Object.defineProperty(object, "hidden", { value: 9 });
Object.defineProperty(object, "fixed", {
    value: 7,
    enumerable: true,
    writable: false,
    configurable: false
});
Object.defineProperty(object, "open", {
    value: 3,
    enumerable: true,
    writable: true,
    configurable: true
});

let fixedDescriptor = Object.getOwnPropertyDescriptor(object, "fixed");
let hiddenDescriptor = Object.getOwnPropertyDescriptor(object, "hidden");
let missingDescriptor = Object.getOwnPropertyDescriptor(object, "missing");
object.fixed = 8;
let deleteFixed = delete object.fixed;
let deleteHidden = delete object.hidden;
let deleteOpen = delete object.open;
let keys = Object.keys(object);
let child = { __proto__: object, own: 5 };

print(
    typeof Object.getOwnPropertyDescriptor,
    Object.getOwnPropertyDescriptor.name,
    Object.getOwnPropertyDescriptor.length,
    typeof Object.defineProperty,
    Object.defineProperty.name,
    Object.defineProperty.length,
    typeof Object.keys,
    Object.keys.name,
    Object.keys.length,
    typeof Object.hasOwn,
    Object.hasOwn.name,
    Object.hasOwn.length
);
print(
    fixedDescriptor.value,
    fixedDescriptor.enumerable,
    fixedDescriptor.writable,
    fixedDescriptor.configurable,
    object.fixed,
    deleteFixed
);
print(
    hiddenDescriptor.value,
    hiddenDescriptor.enumerable,
    hiddenDescriptor.writable,
    hiddenDescriptor.configurable,
    deleteHidden,
    missingDescriptor
);
print(
    keys.length,
    keys[0],
    keys[1],
    Object.hasOwn(object, "fixed"),
    Object.hasOwn(child, "fixed"),
    "fixed" in child,
    Object.hasOwn(child, "own"),
    deleteOpen,
    "keys:" + objectKeys
);
