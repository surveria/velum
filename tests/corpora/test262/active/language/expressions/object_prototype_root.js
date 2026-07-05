let object = {};
let root = object.__proto__;
let Camera = function Camera() {};
let nullProto = { __proto__: null };
let primitiveProto = { __proto__: 7 };

if (object.__proto__ === null) {
    throw new Test262Error("ordinary object should have Object.prototype");
}
if (root.__proto__ !== null) {
    throw new Test262Error("Object.prototype should have a null prototype");
}
if (Camera.prototype.__proto__ !== root) {
    throw new Test262Error("function prototype object should inherit from Object.prototype");
}

let rootKeys = "";
for (let key in root) {
    rootKeys = rootKeys + key + ";";
}
if (rootKeys !== "") {
    throw new Test262Error("Object.prototype constructor should not be enumerable");
}

let deleted = delete Camera.prototype.constructor;
if (!deleted || !("constructor" in Camera.prototype)) {
    throw new Test262Error("function prototype should inherit constructor after deleting own constructor");
}
if (!("constructor" in object) || !("constructor" in primitiveProto)) {
    throw new Test262Error("ordinary objects should inherit constructor");
}
if ("constructor" in nullProto || nullProto.__proto__ !== null) {
    throw new Test262Error("null-prototype object should not inherit Object.prototype");
}

42
