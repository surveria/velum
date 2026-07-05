let stringConstructor = String;
let constructed = new String("camera");
let emptyObject = new String();
let originalPrototype = String.prototype;
String.prototype = null;
let prototypeStayed = String.prototype === originalPrototype &&
    (new String("x")).__proto__ === originalPrototype;

let constructorKeys = "";
for (let key in String) {
    constructorKeys = constructorKeys + key + ";";
}

let prototypeKeys = "";
for (let key in String.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

let boxedKeys = "";
for (let key in constructed) {
    boxedKeys = boxedKeys + key + ";";
}

let primitiveKeys = "";
for (let key in "go") {
    primitiveKeys = primitiveKeys + key + ";";
}

if (
    stringConstructor !== String ||
    typeof String !== "function" ||
    String.name !== "String" ||
    String.length !== 1 ||
    String.prototype.__proto__ !== Object.prototype ||
    String.prototype.constructor.prototype !== String.prototype ||
    constructed.__proto__ !== String.prototype ||
    constructed.constructor !== String ||
    typeof constructed !== "object" ||
    !prototypeStayed ||
    constructorKeys !== "" ||
    prototypeKeys !== "" ||
    boxedKeys !== "0;1;2;3;4;5;" ||
    primitiveKeys !== "0;1;" ||
    String() !== "" ||
    String(null) !== "null" ||
    String(undefined) !== "undefined" ||
    String(true) !== "true" ||
    String(false) !== "false" ||
    String(42) !== "42" ||
    String(Object()) !== "[object Object]" ||
    constructed.length !== 6 ||
    constructed[0] !== "c" ||
    constructed[5] !== "a" ||
    constructed[6] !== undefined ||
    emptyObject.length !== 0 ||
    String("front").length !== 5 ||
    String("front")[1] !== "r" ||
    !("1" in "go") ||
    "2" in "go"
) {
    throw new Test262Error("String constructor behavior was unexpected");
}

42
