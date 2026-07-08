let original = globalThis;
let descriptor = Object.getOwnPropertyDescriptor(this, "globalThis");
let names = Object.getOwnPropertyNames(this);
let builtinAccess =
    globalThis.Object === Object &&
    globalThis.Array === Array &&
    globalThis.Math === Math &&
    globalThis.parseInt === parseInt;

globalThis.marker = 40;
let propertyAccess =
    this.marker === 40 &&
    globalThis.marker === 40 &&
    "marker" in globalThis;

let shadowed = (function() {
    let globalThis = 2;
    return globalThis;
})();

globalThis = { value: 42 };
let assignmentAccess =
    this.globalThis.value === 42 &&
    globalThis.value === 42;
this.globalThis = original;

print(
    this === globalThis,
    globalThis.globalThis === globalThis,
    descriptor.writable,
    descriptor.enumerable,
    descriptor.configurable,
    names.includes("globalThis"),
    builtinAccess,
    propertyAccess,
    shadowed,
    assignmentAccess
);
