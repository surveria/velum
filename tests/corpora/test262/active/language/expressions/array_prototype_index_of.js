let values = [1, 2, 3, 2, undefined, null, "2"];
let sparse = Array(3);
sparse[2] = "tail";
let withUndefined = Array(2);
withUndefined[1] = undefined;

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedIndex = inherited.indexOf("proto-one");
let inheritedUndefined = inherited.indexOf(undefined);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extra = [7].indexOf(7, 0, marker());

values.indexOf(2) === 1 &&
    values.indexOf(2, 2) === 3 &&
    values.indexOf(2, -4) === 3 &&
    values.indexOf(9) === -1 &&
    values.indexOf(1, 99) === -1 &&
    values.indexOf("2") === 6 &&
    values.indexOf(undefined) === 4 &&
    values.indexOf(null) === 5 &&
    values.indexOf(2, "2") === 3 &&
    values.indexOf(2, 2.9) === 3 &&
    values.indexOf(1, -99) === 0 &&
    sparse.indexOf(undefined) === -1 &&
    sparse.indexOf("tail") === 2 &&
    sparse.indexOf("tail", -1) === 2 &&
    withUndefined.indexOf(undefined) === 1 &&
    inheritedIndex === 1 &&
    inheritedUndefined === -1 &&
    side === 42 &&
    extra === 0 &&
    [0, 1].indexOf(1, true) === 1 &&
    [0].indexOf(0, null) === 0 &&
    [undefined].indexOf() === 0 &&
    typeof Array.prototype.indexOf === "function" &&
    Array.prototype.indexOf.name === "indexOf" &&
    Array.prototype.indexOf.length === 1
    ? 42
    : 0
