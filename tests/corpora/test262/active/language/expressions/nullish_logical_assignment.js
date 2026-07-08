let order = "";
let mark = function(label, value) {
    order += label;
    return value;
};

let followsNull = mark("a", null) ?? mark("b", 7);
let keepsZero = mark("c", 0) ?? mark("d", 9);
let keepsFalse = mark("e", false) ?? mark("f", true);
let followsUndefined = mark("g", undefined) ?? mark("h", "fallback");
if (
    order !== "abcegh" ||
    followsNull !== 7 ||
    keepsZero !== 0 ||
    keepsFalse !== false ||
    followsUndefined !== "fallback"
) {
    throw new Test262Error("nullish coalescing did not preserve short-circuit semantics");
}

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
if (
    andValue !== 6 ||
    skippedAnd !== 0 ||
    orValue !== 5 ||
    skippedOr !== 6 ||
    nullishValue !== 4 ||
    undefinedValue !== 3 ||
    truthy !== 6 ||
    falsy !== 5 ||
    empty !== 4 ||
    missing !== 3 ||
    order !== "abcegh"
) {
    throw new Test262Error("logical assignment did not preserve binding semantics");
}

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
if (
    order !== "abceghkslot;rslot;kkeep;kempty;rempty;kyes;ryes;kno;" ||
    storedOr !== 10 ||
    keptOr !== "value" ||
    storedNullish !== 11 ||
    storedAnd !== 12 ||
    keptAnd !== false ||
    target.slot !== 10 ||
    target.keep !== "value" ||
    target.empty !== 11 ||
    target.yes !== 12 ||
    target.no !== false
) {
    throw new Test262Error("logical assignment did not preserve property semantics");
}

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
if (
    indexOrder !== "i0;vzero;i1;vnull;i2;vtrue;i3;" ||
    arrayOr !== 21 ||
    arrayNullish !== 22 ||
    arrayAnd !== 23 ||
    arraySkip !== false ||
    values[0] !== 21 ||
    values[1] !== 22 ||
    values[2] !== 23 ||
    values[3] !== false
) {
    throw new Test262Error("logical assignment did not preserve computed property semantics");
}

let parenthesizedHead = (false || null) ?? 31;
let parenthesizedTail = null ?? (false || 32);
if (parenthesizedHead !== 31 || parenthesizedTail !== 32) {
    throw new Test262Error("parenthesized nullish and logical expressions did not evaluate");
}

42
