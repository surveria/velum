let early = [];
let arrayConstructor = Array;
let created = Array();
let constructed = new Array();
let withElements = Array("front", 42);
let withLength = Array(3);
let originalPrototype = Array.prototype;
Array.prototype = null;
let prototypeStayed = Array.prototype === originalPrototype &&
    [].__proto__ === originalPrototype;

let constructorKeys = "";
for (let key in Array) {
    constructorKeys = constructorKeys + key + ";";
}

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print(
    typeof Array,
    Array.name,
    Array.length,
    Array.prototype.constructor === Array
);
print(
    arrayConstructor === Array,
    early.__proto__ === Array.prototype,
    Array.prototype.__proto__ === Object.prototype,
    early.constructor === Array,
    prototypeStayed
);
print(
    created.length,
    constructed.length,
    withElements.length,
    withElements[0],
    withElements[1],
    withLength.length,
    withLength[0]
);
print("keys:" + constructorKeys + "|" + prototypeKeys);
