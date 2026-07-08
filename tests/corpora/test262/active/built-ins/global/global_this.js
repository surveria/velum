let original = globalThis;
let descriptor = Object.getOwnPropertyDescriptor(this, "globalThis");
let names = Object.getOwnPropertyNames(this);
let builtinAccess =
    globalThis.Object === Object &&
    globalThis.Array === Array &&
    globalThis.Math === Math &&
    globalThis.parseInt === parseInt;

globalThis.test262Marker = 40;
let propertyAccess =
    this.test262Marker === 40 &&
    globalThis.test262Marker === 40 &&
    "test262Marker" in globalThis;

let shadowed = (function() {
    let globalThis = 2;
    return globalThis;
})();

globalThis = { value: 42 };
let assignmentAccess =
    this.globalThis.value === 42 &&
    globalThis.value === 42;
this.globalThis = original;

if (this !== globalThis) {
    throw new Test262Error("top-level this is not globalThis");
}
if (globalThis.globalThis !== globalThis) {
    throw new Test262Error("globalThis property does not point at the global object");
}
if (descriptor.writable !== true) {
    throw new Test262Error("globalThis is not writable");
}
if (descriptor.enumerable !== false) {
    throw new Test262Error("globalThis is enumerable");
}
if (descriptor.configurable !== true) {
    throw new Test262Error("globalThis is not configurable");
}
if (!names.includes("globalThis")) {
    throw new Test262Error("globalThis is missing from own property names");
}
if (!builtinAccess) {
    throw new Test262Error("global built-ins are not visible through globalThis");
}
if (!propertyAccess) {
    throw new Test262Error("global object property access failed");
}
if (shadowed !== 2) {
    throw new Test262Error("lexical globalThis shadowing failed");
}
if (!assignmentAccess) {
    throw new Test262Error("globalThis assignment did not stay property-coherent");
}

42
