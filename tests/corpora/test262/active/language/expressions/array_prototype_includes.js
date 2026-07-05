let nan = 0 / 0;
let values = [1, 2, 3, 2, undefined, null, "2", nan, -0];
let sparse = Array(3);
sparse[2] = "tail";
let withUndefined = Array(2);
withUndefined[1] = undefined;

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedMatch = inherited.includes("proto-one");
let inheritedUndefined = inherited.includes(undefined);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extra = [7].includes(7, 0, marker());

values.includes(2) &&
    values.includes(2, 2) &&
    !values.includes(2, 4) &&
    !values.includes(9) &&
    values.includes(null, -4) &&
    values.includes("2") &&
    values.includes(undefined) &&
    values.includes(null) &&
    values.includes(2, "2") &&
    values.includes(2, 2.9) &&
    values.includes(1, -99) &&
    values.includes(0 / 0) &&
    values.includes(0) &&
    !values.includes(1, 99) &&
    sparse.includes(undefined) &&
    sparse.includes("tail") &&
    sparse.includes("tail", -1) &&
    withUndefined.includes(undefined) &&
    inheritedMatch &&
    inheritedUndefined &&
    side === 42 &&
    extra &&
    [0, 1].includes(1, true) &&
    [0].includes(0, null) &&
    [undefined].includes() &&
    typeof Array.prototype.includes === "function" &&
    Array.prototype.includes.name === "includes" &&
    Array.prototype.includes.length === 1
    ? 42
    : 0
