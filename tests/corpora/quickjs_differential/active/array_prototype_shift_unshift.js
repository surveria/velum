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

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("shift", first, values.length, values[0], values[1], values[2], side);
print("sparse", sparseFirst, sparse.length, "0" in sparse, sparse[0], sparse[1]);
print("inherited", inheritedShiftFirst, inheritedShift.length, inheritedShiftValue);
print("unshift", newLength, sameLength, base.length, base[0], base[1], base[2]);
print("holes", sparseLength, "1" in sparseUnshift, sparseUnshift.join("|"));
print("inherited-unshift", inheritedUnshiftLength, inheritedUnshiftJoin, emptyShift);
print(
    "meta",
    typeof Array.prototype.shift,
    Array.prototype.shift.name,
    Array.prototype.shift.length,
    typeof Array.prototype.unshift,
    Array.prototype.unshift.name,
    Array.prototype.unshift.length
);
print("keys:" + prototypeKeys);
print("in", "shift" in base, "unshift" in base);
