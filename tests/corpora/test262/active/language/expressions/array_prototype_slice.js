let values = [1, 2, 3, 4];
let middle = values.slice(1, 3);
let negative = values.slice(-3, -1);
let startOnly = values.slice(2);
let overflow = values.slice(99);
let reversed = values.slice(3, 1);

let sparse = Array(4);
sparse[1] = "one";
sparse[3] = "three";
let sparseCopy = sparse.slice(1, 4);

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedCopy = inherited.slice(0, 3);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let sideCopy = [7].slice(0, 1, marker());

let coercedNull = [1, 2, 3].slice(null, "2");
let coercedBool = [1, 2, 3].slice(false, true);

middle.join("|") === "2|3" &&
    negative.join("|") === "2|3" &&
    startOnly.join("|") === "3|4" &&
    overflow.length === 0 &&
    reversed.length === 0 &&
    values.length === 4 &&
    values[1] === 2 &&
    sparseCopy.length === 3 &&
    sparseCopy[0] === "one" &&
    !("1" in sparseCopy) &&
    sparseCopy[1] === undefined &&
    sparseCopy[2] === "three" &&
    sparse.join("|") === "|one||three" &&
    inheritedCopy.length === 3 &&
    inheritedCopy[0] === undefined &&
    inheritedCopy[1] === "proto-one" &&
    inheritedCopy[2] === "tail" &&
    ("1" in inheritedCopy) &&
    coercedNull.join("|") === "1|2" &&
    coercedBool.join("|") === "1" &&
    side === 42 &&
    sideCopy.join("|") === "7" &&
    Array.prototype.slice.name === "slice" &&
    Array.prototype.slice.length === 2 &&
    ("slice" in values) ? 42 : 0
