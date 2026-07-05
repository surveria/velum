let booleanConstructor = Boolean;
let constructedFalse = new Boolean(false);
let constructedTrue = new Boolean(1);
let originalPrototype = Boolean.prototype;
Boolean.prototype = null;
let prototypeStayed = Boolean.prototype === originalPrototype &&
    (new Boolean()).__proto__ === originalPrototype;

let constructorKeys = "";
for (let key in Boolean) {
    constructorKeys = constructorKeys + key + ";";
}

let prototypeKeys = "";
for (let key in Boolean.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

let shadowResult = 0;
{
    let Boolean = function(value) {
        return value + 35;
    };
    shadowResult = Boolean(7);
}

if (
    booleanConstructor !== Boolean ||
    typeof Boolean !== "function" ||
    Boolean.name !== "Boolean" ||
    Boolean.length !== 1 ||
    Boolean.prototype.__proto__ !== Object.prototype ||
    Boolean.prototype.constructor.prototype !== Boolean.prototype ||
    constructedFalse.__proto__ !== Boolean.prototype ||
    constructedTrue.__proto__ !== Boolean.prototype ||
    constructedFalse.constructor !== Boolean ||
    typeof constructedFalse !== "object" ||
    !prototypeStayed ||
    constructorKeys !== "" ||
    prototypeKeys !== "" ||
    shadowResult !== 42 ||
    Boolean() !== false ||
    Boolean(false) !== false ||
    Boolean(0) !== false ||
    Boolean("") !== false ||
    Boolean(null) !== false ||
    Boolean(undefined) !== false ||
    Boolean(true) !== true ||
    Boolean(1) !== true ||
    Boolean("camera") !== true ||
    Boolean(Object()) !== true ||
    Boolean(constructedFalse) !== true
) {
    throw new Test262Error("Boolean constructor behavior was unexpected");
}

42
