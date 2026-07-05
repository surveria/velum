let plain = {};
let objectConstructor = Object;
let created = Object();
let constructed = new Object();
let returned = Object(plain);
let returnedFromNew = new Object(plain);
let originalPrototype = Object.prototype;
Object.prototype = null;
let prototypeStayed = Object.prototype === originalPrototype &&
    (new Object()).__proto__ === originalPrototype;

let constructorKeys = "";
for (let key in Object) {
    constructorKeys = constructorKeys + key + ";";
}

let prototypeKeys = "";
for (let key in Object.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print(
    typeof Object,
    Object.name,
    Object.length,
    Object.prototype.constructor === Object
);
print(
    created.__proto__ === Object.prototype,
    constructed.__proto__ === Object.prototype,
    returned === plain,
    returnedFromNew === plain,
    prototypeStayed
);
print("keys:" + constructorKeys + "|" + prototypeKeys);

objectConstructor === Object &&
    Object.prototype.__proto__ === null &&
    Object.prototype.constructor.prototype === Object.prototype &&
    plain.constructor === Object &&
    created.__proto__ === Object.prototype &&
    constructed.__proto__ === Object.prototype &&
    returned === plain &&
    returnedFromNew === plain &&
    prototypeStayed &&
    constructorKeys === "" &&
    prototypeKeys === "" ? 42 : 0
