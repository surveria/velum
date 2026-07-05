let values = [1, 2, 3, 2, undefined, null, "2"];
let firstTwo = values.indexOf(2);
let nextTwo = values.indexOf(2, 2);
let fromNegative = values.indexOf(2, -4);
let missing = values.indexOf(9);
let fromTooLarge = values.indexOf(1, 99);
let stringTwo = values.indexOf("2");
let undefinedIndex = values.indexOf(undefined);
let nullIndex = values.indexOf(null);
let stringStart = values.indexOf(2, "2");
let fractionStart = values.indexOf(2, 2.9);
let veryNegative = values.indexOf(1, -99);

let sparse = Array(3);
sparse[2] = "tail";
let holeUndefined = sparse.indexOf(undefined);
let tailIndex = sparse.indexOf("tail");
let tailFromEnd = sparse.indexOf("tail", -1);

let withUndefined = Array(2);
withUndefined[1] = undefined;
let ownUndefined = withUndefined.indexOf(undefined);

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

let boolStart = [0, 1].indexOf(1, true);
let nullStart = [0].indexOf(0, null);
let missingSearch = [undefined].indexOf();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("indexOf", firstTwo, nextTwo, fromNegative, missing, fromTooLarge, stringTwo);
print("values", undefinedIndex, nullIndex, stringStart, fractionStart, veryNegative);
print("sparse", holeUndefined, tailIndex, tailFromEnd, ownUndefined);
print("inherited", inheritedIndex, inheritedUndefined, side, extra);
print("coerced", boolStart, nullStart, missingSearch);
print("meta", typeof Array.prototype.indexOf, Array.prototype.indexOf.name, Array.prototype.indexOf.length);
print("keys:" + prototypeKeys);
print("in", "indexOf" in values);
