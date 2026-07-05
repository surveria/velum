let values = [1, 2, 3, 2, undefined, null, "2"];
let lastTwo = values.lastIndexOf(2);
let beforeLast = values.lastIndexOf(2, 2);
let fromNegative = values.lastIndexOf(2, -4);
let missing = values.lastIndexOf(9);
let fromTooLarge = values.lastIndexOf(1, 99);
let stringTwo = values.lastIndexOf("2");
let undefinedIndex = values.lastIndexOf(undefined);
let nullIndex = values.lastIndexOf(null);
let stringStart = values.lastIndexOf(2, "2");
let fractionStart = values.lastIndexOf(2, 2.9);
let veryNegative = values.lastIndexOf(1, -99);
let undefinedStart = values.lastIndexOf(2, undefined);

let sparse = Array(3);
sparse[2] = "tail";
let holeUndefined = sparse.lastIndexOf(undefined);
let tailIndex = sparse.lastIndexOf("tail");
let tailBeforeEnd = sparse.lastIndexOf("tail", 1);
let tailFromEnd = sparse.lastIndexOf("tail", -1);

let withUndefined = Array(2);
withUndefined[1] = undefined;
let ownUndefined = withUndefined.lastIndexOf(undefined);

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

let boolStart = [0, 1].lastIndexOf(1, true);
let nullStart = [0].lastIndexOf(0, null);
let missingSearch = [undefined].lastIndexOf();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("lastIndexOf", lastTwo, beforeLast, fromNegative, missing, fromTooLarge, stringTwo);
print("values", undefinedIndex, nullIndex, stringStart, fractionStart, veryNegative, undefinedStart);
print("sparse", holeUndefined, tailIndex, tailBeforeEnd, tailFromEnd, ownUndefined);
print("inherited", inheritedIndex, inheritedUndefined, side, extra);
print("coerced", boolStart, nullStart, missingSearch);
print("meta", typeof Array.prototype.lastIndexOf, Array.prototype.lastIndexOf.name, Array.prototype.lastIndexOf.length);
print("keys:" + prototypeKeys);
print("in", "lastIndexOf" in values);
