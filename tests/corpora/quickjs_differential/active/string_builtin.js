let constructed = new String("camera");
let emptyObject = new String();

let boxedKeys = "";
for (let key in constructed) {
    boxedKeys = boxedKeys + key + ";";
}

let primitiveKeys = "";
for (let key in "go") {
    primitiveKeys = primitiveKeys + key + ";";
}

print(
    typeof String,
    String.name,
    String.length,
    String.prototype.constructor === String
);
print(
    String(),
    String(null),
    String(undefined),
    String(true),
    String(false),
    String(42),
    String(Object())
);
print(
    constructed.length,
    constructed[0],
    constructed[5],
    emptyObject.length,
    String("front").length,
    String("front")[1]
);
print("keys:" + boxedKeys + "|" + primitiveKeys);
