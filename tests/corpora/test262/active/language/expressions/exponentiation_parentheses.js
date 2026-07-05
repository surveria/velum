let power = 2 ** 3 ** 2;
if (power !== 512) {
    throw new Test262Error("exponentiation did not associate to the right");
}

let grouped = (-2) ** 2;
let negated = -(2 ** 2);
if (grouped !== 4 || negated !== -4) {
    throw new Test262Error("parenthesized exponentiation produced an unexpected value");
}

let value = 2;
let assigned = (value) **= 3;
let old = (value)++;
let current = ++(value);
if (assigned !== 8 || old !== 8 || current !== 10 || value !== 10) {
    throw new Test262Error("parenthesized binding target did not update correctly");
}

let target = { slot: 4 };
let propPower = (target.slot) **= 2;
if (propPower !== 16 || target.slot !== 16) {
    throw new Test262Error("parenthesized property target did not update correctly");
}

let missingType = typeof (missing);
let deleteMissing = delete (missing);
let object = { value: 1 };
let deleteProperty = delete (object.value);
if (
    missingType !== "undefined" ||
    deleteMissing !== true ||
    deleteProperty !== true ||
    object.value !== undefined
) {
    throw new Test262Error("parenthesized typeof/delete semantics were unexpected");
}

let choose = function(value) {
    return value;
};
let called = (choose)(42);
if (called !== 42) {
    throw new Test262Error("parenthesized function callee did not call correctly");
}

42
