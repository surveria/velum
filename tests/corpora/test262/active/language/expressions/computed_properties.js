let object = {
    first: 40,
    nested: { value: 2 },
};

let key = "first";
if (object[key] !== 40) {
    throw new Test262Error("computed property read returned an unexpected value");
}

if (object["nested"]["value"] !== 2) {
    throw new Test262Error("chained computed property read returned an unexpected value");
}

let assigned = object[key] = object[key] + object["nested"].value;
if (assigned !== 42 || object.first !== 42) {
    throw new Test262Error("computed property assignment did not store the value");
}

object[1] = 40;
object[true] = object["1"] + 2;
if (object["true"] !== 42) {
    throw new Test262Error("computed property key conversion returned an unexpected value");
}

let order = "";
let target = {};
let property = function() {
    order = order + "k";
    return "value";
};
let payload = function() {
    order = order + "v";
    return 42;
};
target[property()] = payload();
if (order !== "kv" || target.value !== 42) {
    throw new Test262Error("computed property assignment used an unexpected evaluation order");
}

let caught = "";
try {
    missing = missing;
} catch (error) {
    caught = error["name"] + ":" + error["message"];
}

if (caught !== "ReferenceError:'missing' is not defined") {
    throw new Test262Error("computed access did not read error object properties");
}

42
