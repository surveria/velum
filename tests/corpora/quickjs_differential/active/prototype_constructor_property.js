let Camera = function Camera() {};
let Replacement = function Replacement() {};

let beforeKeys = "";
for (let key in Camera.prototype) {
    beforeKeys = beforeKeys + key + ";";
}
let beforeHas = "constructor" in Camera.prototype;
let beforeSame = Camera.prototype.constructor === Camera;

Camera.prototype.constructor = Replacement;
let afterSetKeys = "";
for (let key in Camera.prototype) {
    afterSetKeys = afterSetKeys + key + ";";
}
let afterSetSame = Camera.prototype.constructor === Replacement;

let deleted = delete Camera.prototype.constructor;

Camera.prototype.constructor = Camera;
let afterReaddKeys = "";
for (let key in Camera.prototype) {
    afterReaddKeys = afterReaddKeys + key + ";";
}

print("keys:" + beforeKeys + "|" + afterSetKeys + "|" + afterReaddKeys);
print(beforeHas, beforeSame, afterSetSame, deleted);
