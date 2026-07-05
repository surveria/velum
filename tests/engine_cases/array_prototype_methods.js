let values = [1];
let firstPush = values.push(2, 3);
let secondPush = values.push();
let popped = values.pop("ignored");
let afterPopLength = values.length;
delete values[1];
let hole = values.pop();
let last = values.pop();
let empty = values.pop();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
[7].pop(marker());

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

let arrayKeys = "";
for (let key in [4, 5]) {
    arrayKeys = arrayKeys + key + ";";
}

print(
    "methods",
    typeof Array.prototype.push,
    Array.prototype.push.name,
    Array.prototype.push.length,
    typeof Array.prototype.pop,
    Array.prototype.pop.name,
    Array.prototype.pop.length
);
print(
    "values",
    firstPush,
    secondPush,
    popped,
    afterPopLength,
    hole,
    last,
    empty,
    values.length,
    side
);
print("keys:" + prototypeKeys + "|" + arrayKeys);
print("in", "push" in values, "pop" in values);

firstPush === 3 &&
    secondPush === 3 &&
    popped === 3 &&
    afterPopLength === 2 &&
    hole === undefined &&
    last === 1 &&
    empty === undefined &&
    values.length === 0 &&
    side === 42 &&
    prototypeKeys === "" &&
    arrayKeys === "0;1;" &&
    ("push" in values) &&
    ("pop" in values) ? 42 : 0
