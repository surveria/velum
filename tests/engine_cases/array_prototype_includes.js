let nan = 0 / 0;
let values = [1, 2, 3, 2, undefined, null, "2", nan, -0];
let hasTwo = values.includes(2);
let nextTwo = values.includes(2, 2);
let lateTwo = values.includes(2, 4);
let missing = values.includes(9);
let fromNegative = values.includes(null, -4);
let stringTwo = values.includes("2");
let undefinedMatch = values.includes(undefined);
let nullMatch = values.includes(null);
let stringStart = values.includes(2, "2");
let fractionStart = values.includes(2, 2.9);
let veryNegative = values.includes(1, -99);
let nanMatch = values.includes(0 / 0);
let zeroMatch = values.includes(0);
let fromTooLarge = values.includes(1, 99);

let sparse = Array(3);
sparse[2] = "tail";
let holeUndefined = sparse.includes(undefined);
let tailIndex = sparse.includes("tail");
let tailFromEnd = sparse.includes("tail", -1);
let sparseMissing = sparse.includes("missing");

let withUndefined = Array(2);
withUndefined[1] = undefined;
let ownUndefined = withUndefined.includes(undefined);

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

let boolStart = [0, 1].includes(1, true);
let nullStart = [0].includes(0, null);
let missingSearch = [undefined].includes();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("includes", hasTwo, nextTwo, lateTwo, missing, fromNegative, stringTwo);
print("values", undefinedMatch, nullMatch, stringStart, fractionStart, veryNegative, nanMatch, zeroMatch, fromTooLarge);
print("sparse", holeUndefined, tailIndex, tailFromEnd, ownUndefined, sparseMissing);
print("inherited", inheritedMatch, inheritedUndefined, side, extra);
print("coerced", boolStart, nullStart, missingSearch);
print("meta", typeof Array.prototype.includes, Array.prototype.includes.name, Array.prototype.includes.length);
print("keys:" + prototypeKeys);
print("in", "includes" in values);

hasTwo &&
    nextTwo &&
    !lateTwo &&
    !missing &&
    fromNegative &&
    stringTwo &&
    undefinedMatch &&
    nullMatch &&
    stringStart &&
    fractionStart &&
    veryNegative &&
    nanMatch &&
    zeroMatch &&
    !fromTooLarge &&
    holeUndefined &&
    tailIndex &&
    tailFromEnd &&
    ownUndefined &&
    !sparseMissing &&
    inheritedMatch &&
    inheritedUndefined &&
    side === 42 &&
    extra &&
    boolStart &&
    nullStart &&
    missingSearch &&
    typeof Array.prototype.includes === "function" &&
    Array.prototype.includes.name === "includes" &&
    Array.prototype.includes.length === 1 &&
    prototypeKeys === "" &&
    ("includes" in values) ? 42 : 0
