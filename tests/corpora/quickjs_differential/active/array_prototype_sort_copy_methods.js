let numericSort = [3, 1, 2, 10, 5];
numericSort.sort((a, b) => a - b);
print("numeric", numericSort.join("|"));

let defaultSort = [3, 1, 2, 10, 20, 100];
defaultSort.sort();
print("default", defaultSort.join("|"));

let mixedSort = ["banana", "apple", "cherry", "Apple"];
mixedSort.sort();
print("mixed", mixedSort.join("|"));

let undefinedSort = ["b", undefined, "a", undefined, "c"];
undefinedSort.sort();
print("undef", undefinedSort.length, undefinedSort[0], undefinedSort[1], undefinedSort[2], undefinedSort[3], undefinedSort[4]);

let spliceTarget = [1, 2, 3, 4, 5];
let spliceRemoved = spliceTarget.splice(1, 2, "a", "b", "c");
print("splice", spliceTarget.join("|"), spliceRemoved.join("|"));

let spliceGrow = [1, 2, 3];
let spliceGrowRemoved = spliceGrow.splice(1, 0, "x", "y");
print("splice-grow", spliceGrow.join("|"), spliceGrowRemoved.length);

let spliceNegative = [1, 2, 3, 4, 5];
let spliceNegativeRemoved = spliceNegative.splice(-2, 1);
print("splice-neg", spliceNegative.join("|"), spliceNegativeRemoved.join("|"));

print("fill", [1, 2, 3, 4, 5].fill(9, 1, 3).join("|"));
print("fill-neg", [1, 2, 3, 4, 5].fill(0, -2).join("|"));
print("copy", [1, 2, 3, 4, 5].copyWithin(0, 3).join("|"));
print("copy-overlap", [1, 2, 3, 4, 5].copyWithin(1, 0, 3).join("|"));
print("copy-neg", [1, 2, 3, 4, 5].copyWithin(-2, 0).join("|"));

let source = [10, 20, 30, 40];
print("at", source.at(0), source.at(-1), source.at(-4), source.at(4), source.at(-5));

print("findLast", [1, 2, 3, 4, 5].findLast((value) => value % 2 === 0));
print("findLastIndex", [1, 2, 3, 4, 5].findLastIndex((value) => value % 2 === 0));
print("findLast-none", [1, 3, 5].findLast((value) => value % 2 === 0));
print("findLastIndex-none", [1, 3, 5].findLastIndex((value) => value % 2 === 0));

let original = [3, 1, 2];
let sorted = original.toSorted((a, b) => a - b);
print("toSorted", sorted.join("|"), original.join("|"));

let toRev = [1, 2, 3, 4];
print("toReversed", toRev.toReversed().join("|"), toRev.join("|"));

let toSpl = [1, 2, 3, 4, 5];
let toSplResult = toSpl.toSpliced(1, 2, "a", "b");
print("toSpliced", toSplResult.join("|"), toSpl.join("|"));

let withSource = [1, 2, 3, 4];
print("with", withSource.with(1, 99).join("|"), withSource.with(-1, 88).join("|"), withSource.join("|"));

let rangeError = false;
try {
    [1, 2, 3].with(10, 0);
} catch (error) {
    rangeError = error instanceof RangeError;
}
print("with-range", rangeError);

let sortError = false;
try {
    [1, 2, 3].sort(42);
} catch (error) {
    sortError = error instanceof TypeError;
}
print("sort-type", sortError);

print(
    "meta",
    Array.prototype.sort.length,
    Array.prototype.splice.length,
    Array.prototype.fill.length,
    Array.prototype.copyWithin.length,
    Array.prototype.at.length,
    Array.prototype.findLast.length,
    Array.prototype.findLastIndex.length,
    Array.prototype.toSorted.length,
    Array.prototype.toReversed.length,
    Array.prototype.toSpliced.length,
    Array.prototype.with.length
);

let generic = { length: 3, 0: "z", 1: "a", 2: "m" };
Array.prototype.sort.call(generic);
print("generic-sort", generic[0], generic[1], generic[2], generic.length);
