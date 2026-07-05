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

print(
    f.tag,
    functionKeys.length,
    functionKeys[0],
    tagDescriptor.enumerable,
    tagDescriptor.writable,
    tagDescriptor.configurable,
    deleteTag
);
print(
    hiddenDescriptor.value,
    hiddenDescriptor.enumerable,
    nameDescriptor.value,
    nameDescriptor.configurable,
    lengthDescriptor.value,
    lengthDescriptor.configurable
);
print(
    Object.keys.tag,
    nativeKeys.length,
    nativeKeys[0],
    nativeTagDescriptor.enumerable,
    nativeTagDescriptor.writable,
    nativeTagDescriptor.configurable,
    deleteNativeTag
);
print(
    nativeNameDescriptor.value,
    nativeNameDescriptor.configurable,
    nativeLengthDescriptor.value,
    nativeLengthDescriptor.configurable
);
