let values = [1, "two", null, undefined, true];
let defaultJoin = values.join();
let dashJoin = values.join("-");
let nullSeparator = [1, 2].join(null);

let sparse = Array(3);
sparse[1] = "middle";
let sparseJoin = sparse.join("|");
let emptyJoin = [].join();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extraIgnored = [7].join(undefined, marker());

Array.prototype[0] = "proto";
let inherited = Array(2).join("|");
delete Array.prototype[0];

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("join", defaultJoin, dashJoin, nullSeparator);
print("sparse", emptyJoin === "", sparseJoin, extraIgnored, side, inherited);
print("meta", typeof Array.prototype.join, Array.prototype.join.name, Array.prototype.join.length);
print("keys:" + prototypeKeys);
print("in", "join" in values);
