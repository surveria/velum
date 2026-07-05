let total = 0;
let root = ({}).__proto__;

for (let index = 0; index < 128; index++) {
    let object = { value: index };
    if (object.__proto__ === root) {
        total += 1;
    }
    if ("constructor" in object) {
        total += 1;
    }

    let Camera = function Camera() {};
    delete Camera.prototype.constructor;
    if ("constructor" in Camera.prototype) {
        total += 1;
    }

    let nullProto = { __proto__: null };
    if (!("constructor" in nullProto)) {
        total += 1;
    }
}

total
