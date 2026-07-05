let Camera = function Camera() {};
let Replacement = function Replacement() {};

let beforeKeys = "";
for (let key in Camera.prototype) {
    beforeKeys = beforeKeys + key + ";";
}
if (beforeKeys !== "") {
    throw new Test262Error("prototype constructor should not be enumerable");
}
if (!("constructor" in Camera.prototype)) {
    throw new Test262Error("prototype constructor should exist");
}
if (Camera.prototype.constructor !== Camera) {
    throw new Test262Error("prototype constructor should point at the function");
}

Camera.prototype.constructor = Replacement;
let afterSetKeys = "";
for (let key in Camera.prototype) {
    afterSetKeys = afterSetKeys + key + ";";
}
if (afterSetKeys !== "") {
    throw new Test262Error("prototype constructor assignment changed enumerability");
}
if (Camera.prototype.constructor !== Replacement) {
    throw new Test262Error("prototype constructor assignment did not update value");
}

let deleted = delete Camera.prototype.constructor;
if (!deleted) {
    throw new Test262Error("prototype constructor delete was unexpected");
}

Camera.prototype.constructor = Camera;
let afterReaddKeys = "";
for (let key in Camera.prototype) {
    afterReaddKeys = afterReaddKeys + key + ";";
}
if (afterReaddKeys !== "constructor;") {
    throw new Test262Error("ordinary assignment should create enumerable constructor");
}

42
