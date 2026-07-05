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

print(
    typeof Boolean,
    Boolean.name,
    Boolean.length,
    Boolean.prototype.constructor === Boolean
);
print(
    Boolean(),
    Boolean(false),
    Boolean(0),
    Boolean(""),
    Boolean(null),
    Boolean(undefined),
    Boolean(true),
    Boolean(1),
    Boolean("camera"),
    Boolean(Object())
);
print(
    typeof constructedFalse,
    constructedFalse.__proto__ === Boolean.prototype,
    constructedFalse.constructor === Boolean,
    Boolean(constructedFalse)
);
print("keys:" + constructorKeys + "|" + prototypeKeys);

booleanConstructor === Boolean &&
    typeof Boolean === "function" &&
    Boolean.name === "Boolean" &&
    Boolean.length === 1 &&
    Boolean.prototype.__proto__ === Object.prototype &&
    Boolean.prototype.constructor.prototype === Boolean.prototype &&
    constructedFalse.__proto__ === Boolean.prototype &&
    constructedTrue.__proto__ === Boolean.prototype &&
    constructedFalse.constructor === Boolean &&
    typeof constructedFalse === "object" &&
    prototypeStayed &&
    constructorKeys === "" &&
    prototypeKeys === "" &&
    shadowResult === 42 &&
    Boolean() === false &&
    Boolean(false) === false &&
    Boolean(0) === false &&
    Boolean("") === false &&
    Boolean(null) === false &&
    Boolean(undefined) === false &&
    Boolean(true) === true &&
    Boolean(1) === true &&
    Boolean("camera") === true &&
    Boolean(Object()) === true &&
    Boolean(constructedFalse) === true ? 42 : 0
