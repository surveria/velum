let constructedFalse = new Boolean(false);

let constructorKeys = "";
for (let key in Boolean) {
    constructorKeys = constructorKeys + key + ";";
}

let prototypeKeys = "";
for (let key in Boolean.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
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
