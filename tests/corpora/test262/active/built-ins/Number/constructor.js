let numberConstructor = Number;
let constructed = new Number("7");
let originalPrototype = Number.prototype;
Number.prototype = null;
let prototypeStayed = Number.prototype === originalPrototype &&
    (new Number()).__proto__ === originalPrototype;

let constructorKeys = "";
for (let key in Number) {
    constructorKeys = constructorKeys + key + ";";
}

let prototypeKeys = "";
for (let key in Number.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

let nan = Number.NaN;
let invalid = Number("front");
let deleteNan = delete Number.NaN;
Number.NaN = 7;

if (
    numberConstructor !== Number ||
    typeof Number !== "function" ||
    Number.name !== "Number" ||
    Number.length !== 1 ||
    Number.prototype.__proto__ !== Object.prototype ||
    Number.prototype.constructor.prototype !== Number.prototype ||
    constructed.__proto__ !== Number.prototype ||
    constructed.constructor !== Number ||
    typeof constructed !== "object" ||
    !prototypeStayed ||
    constructorKeys !== "" ||
    prototypeKeys !== "" ||
    Number() !== 0 ||
    Number(null) !== 0 ||
    Number(true) !== 1 ||
    Number(false) !== 0 ||
    Number(" 42 ") !== 42 ||
    Number("1e2") !== 100 ||
    Number("0x10") !== 16 ||
    Number("0b101") !== 5 ||
    Number("0o10") !== 8 ||
    Number("Infinity") !== Number.POSITIVE_INFINITY ||
    Number("-Infinity") !== Number.NEGATIVE_INFINITY ||
    !(Number.MAX_VALUE > 1e300) ||
    !(Number.MIN_VALUE > 0) ||
    !(Number.EPSILON > 0) ||
    Number.MAX_SAFE_INTEGER !== 9007199254740991 ||
    Number.MIN_SAFE_INTEGER !== -9007199254740991 ||
    !(Number.POSITIVE_INFINITY > Number.MAX_VALUE) ||
    !(Number.NEGATIVE_INFINITY < -Number.MAX_VALUE) ||
    nan === nan ||
    invalid === invalid ||
    Number.NaN === Number.NaN ||
    deleteNan !== false
) {
    throw new Test262Error("Number constructor or static property behavior was unexpected");
}

42
