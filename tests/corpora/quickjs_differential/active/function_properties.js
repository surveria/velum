let named = function namedCamera(left, right) {
    return left + right;
};
let anonymous = [function(one, two, three) {
    return one;
}][0];

print(named.length, named.name, named(40, 2));
print(anonymous.length, anonymous.name === "");
print("length" in named, "name" in named, "missing" in named, named.missing === undefined);

let seen = "";
for (let key in named) {
    seen = seen + key + ";";
}
print(seen, named.length, named.name);
