let object = { present: 1, empty: undefined };
let present = "present" in object;
let empty = "empty" in object;
let absent = "absent" in object;
if (present !== true || empty !== true || absent !== false) {
    throw new Test262Error("object property membership was unexpected");
}

delete object.present;
let deleted = "present" in object;
if (deleted !== false) {
    throw new Test262Error("deleted object property membership was unexpected");
}

let values = [undefined, 2];
values[3] = 4;
let hasZero = 0 in values;
let hasOne = "1" in values;
let hasTwo = 2 in values;
let hasThree = 3 in values;
let hasLength = "length" in values;
if (
    hasZero !== true ||
    hasOne !== true ||
    hasTwo !== false ||
    hasThree !== true ||
    hasLength !== true
) {
    throw new Test262Error("array property membership was unexpected");
}

let key = "slot";
let bag = { slot: 42 };
let precedence = key in bag === true;
if (key in bag !== true || ("slot") in bag !== true || precedence !== true) {
    throw new Test262Error("in operator precedence or computed key was unexpected");
}

42
