let object = {};
let root = object.__proto__;
let Camera = function Camera() {};
let cameraRoot = Camera.prototype.__proto__;
let nullProto = { __proto__: null };
let primitiveProto = { __proto__: 7 };

let rootKeys = "";
for (let key in root) {
    rootKeys = rootKeys + key + ";";
}

let objectKeys = "";
for (let key in object) {
    objectKeys = objectKeys + key + ";";
}

let deleted = delete Camera.prototype.constructor;
let cameraKeys = "";
for (let key in Camera.prototype) {
    cameraKeys = cameraKeys + key + ";";
}

print("root", object.__proto__ === null, root.__proto__ === null, cameraRoot === root);
print(
    "constructor",
    "constructor" in object,
    "constructor" in Camera.prototype,
    "constructor" in primitiveProto,
    "constructor" in nullProto
);
print("keys:" + rootKeys + "|" + objectKeys + "|" + cameraKeys);

object.__proto__ !== null &&
    root.__proto__ === null &&
    cameraRoot === root &&
    deleted &&
    ("constructor" in object) &&
    ("constructor" in Camera.prototype) &&
    ("constructor" in primitiveProto) &&
    !("constructor" in nullProto) &&
    nullProto.__proto__ === undefined &&
    rootKeys === "" &&
    objectKeys === "" &&
    cameraKeys === "" ? 42 : 0
