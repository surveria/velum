let values = [1, 2, 3];
let first = values.shift();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
[9].shift(marker());

let sparse = Array(3);
sparse[2] = "tail";
let sparseFirst = sparse.shift();

Array.prototype[1] = "proto-one";
let inheritedShift = Array(2);
let inheritedShiftFirst = inheritedShift.shift();
let inheritedShiftValue = inheritedShift[0];
delete Array.prototype[1];

let base = [3];
let newLength = base.unshift(1, 2);
let sameLength = base.unshift();

let sparseUnshift = Array(2);
sparseUnshift[1] = "b";
let sparseLength = sparseUnshift.unshift("a");

Array.prototype[0] = "proto-zero";
let inheritedUnshift = Array(1);
let inheritedUnshiftLength = inheritedUnshift.unshift("head");
let inheritedUnshiftJoin = inheritedUnshift.join("|");
delete Array.prototype[0];

let emptyShift = [].shift();

first === 1 &&
    values.length === 2 &&
    values[0] === 2 &&
    values[1] === 3 &&
    values[2] === undefined &&
    side === 42 &&
    sparseFirst === undefined &&
    sparse.length === 2 &&
    !("0" in sparse) &&
    sparse[0] === undefined &&
    sparse[1] === "tail" &&
    inheritedShiftFirst === undefined &&
    inheritedShift.length === 1 &&
    inheritedShiftValue === "proto-one" &&
    newLength === 3 &&
    sameLength === 3 &&
    base.length === 3 &&
    base[0] === 1 &&
    base[1] === 2 &&
    base[2] === 3 &&
    sparseLength === 3 &&
    !("1" in sparseUnshift) &&
    sparseUnshift.join("|") === "a||b" &&
    inheritedUnshiftLength === 2 &&
    inheritedUnshiftJoin === "head|proto-zero" &&
    emptyShift === undefined &&
    Array.prototype.shift.name === "shift" &&
    Array.prototype.shift.length === 0 &&
    Array.prototype.unshift.name === "unshift" &&
    Array.prototype.unshift.length === 1 &&
    ("shift" in base) &&
    ("unshift" in base) ? 42 : 0
