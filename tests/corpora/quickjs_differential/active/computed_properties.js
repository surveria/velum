let object = {
    name: "front-door",
    first: 40,
    nested: { value: 2 },
};

let key = "first";
print(object[key]);
print(object["nested"]["value"]);

let assigned = object[key] = object[key] + object["nested"].value;
print(assigned);
print(object.first);

object[1] = 40;
object[true] = object["1"] + 2;
print(object["true"]);

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
print(order);
print(target.value);

try {
    missing = missing;
} catch (error) {
    print(error["name"]);
    print(error["message"]);
}
