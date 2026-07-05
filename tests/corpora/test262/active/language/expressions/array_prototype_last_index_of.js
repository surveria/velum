let values = [1, 2, 3, 2, undefined, null, "2"];
let sparse = Array(3);
sparse[2] = "tail";
let withUndefined = Array(2);
withUndefined[1] = undefined;

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedIndex = inherited.lastIndexOf("proto-one");
let inheritedUndefined = inherited.lastIndexOf(undefined);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extra = [7].lastIndexOf(7, 0, marker());

values.lastIndexOf(2) === 3 &&
    values.lastIndexOf(2, 2) === 1 &&
    values.lastIndexOf(2, -4) === 3 &&
    values.lastIndexOf(9) === -1 &&
    values.lastIndexOf(1, 99) === 0 &&
    values.lastIndexOf("2") === 6 &&
    values.lastIndexOf(undefined) === 4 &&
    values.lastIndexOf(null) === 5 &&
    values.lastIndexOf(2, "2") === 1 &&
    values.lastIndexOf(2, 2.9) === 1 &&
    values.lastIndexOf(1, -99) === -1 &&
    values.lastIndexOf(2, undefined) === -1 &&
    sparse.lastIndexOf(undefined) === -1 &&
    sparse.lastIndexOf("tail") === 2 &&
    sparse.lastIndexOf("tail", 1) === -1 &&
    sparse.lastIndexOf("tail", -1) === 2 &&
    withUndefined.lastIndexOf(undefined) === 1 &&
    inheritedIndex === 1 &&
    inheritedUndefined === -1 &&
    side === 42 &&
    extra === 0 &&
    [0, 1].lastIndexOf(1, true) === 1 &&
    [0].lastIndexOf(0, null) === 0 &&
    [undefined].lastIndexOf() === 0 &&
    typeof Array.prototype.lastIndexOf === "function" &&
    Array.prototype.lastIndexOf.name === "lastIndexOf" &&
    Array.prototype.lastIndexOf.length === 1
    ? 42
    : 0
