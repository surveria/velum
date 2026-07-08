let order = "";
let mark = function(label, value) {
    order += label;
    return value;
};

let followsNull = mark("a", null) ?? mark("b", 7);
let keepsZero = mark("c", 0) ?? mark("d", 9);
let keepsFalse = mark("e", false) ?? mark("f", true);
let followsUndefined = mark("g", undefined) ?? mark("h", "fallback");
print(order, followsNull, keepsZero, keepsFalse, followsUndefined);

let truthy = 1;
let falsy = 0;
let empty = null;
let missing = undefined;
let andValue = truthy &&= 6;
let skippedAnd = falsy &&= mark("i", 99);
let orValue = falsy ||= 5;
let skippedOr = truthy ||= mark("j", 88);
let nullishValue = empty ??= 4;
let undefinedValue = missing ??= 3;
print(andValue, skippedAnd, orValue, skippedOr, nullishValue, undefinedValue);

let target = { slot: 0, keep: "value", empty: null, yes: true, no: false };
let key = function(name) {
    order += "k" + name + ";";
    return name;
};
let rhs = function(label, value) {
    order += "r" + label + ";";
    return value;
};
let storedOr = target[key("slot")] ||= rhs("slot", 10);
let keptOr = target[key("keep")] ||= rhs("keep", "bad");
let storedNullish = target[key("empty")] ??= rhs("empty", 11);
let storedAnd = target[key("yes")] &&= rhs("yes", 12);
let keptAnd = target[key("no")] &&= rhs("no", 13);
print(storedOr, keptOr, storedNullish, storedAnd, keptAnd);
print(target.slot, target.keep, target.empty, target.yes, target.no);

let values = [0, null, true, false];
let indexOrder = "";
let index = function(value) {
    indexOrder += "i" + value + ";";
    return value;
};
let valueRhs = function(label, value) {
    indexOrder += "v" + label + ";";
    return value;
};
let arrayOr = values[index(0)] ||= valueRhs("zero", 21);
let arrayNullish = values[index(1)] ??= valueRhs("null", 22);
let arrayAnd = values[index(2)] &&= valueRhs("true", 23);
let arraySkip = values[index(3)] &&= valueRhs("false", 24);
print(indexOrder, arrayOr, arrayNullish, arrayAnd, arraySkip);
print(values[0], values[1], values[2], values[3]);

let parenthesizedHead = (false || null) ?? 31;
let parenthesizedTail = null ?? (false || 32);
print(parenthesizedHead, parenthesizedTail);
