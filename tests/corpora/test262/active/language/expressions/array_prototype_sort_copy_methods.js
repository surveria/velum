// Coverage for Array.prototype sort/splice/fill/copyWithin/at/findLast and the
// ES2023 change-by-copy methods. Evaluates to 42 when every observable behavior
// matches the specification, otherwise 0.

let numericSort = [3, 1, 2, 10];
numericSort.sort((a, b) => a - b);

let defaultSort = [3, 1, 2, 10];
defaultSort.sort();

let undefinedSort = ["b", undefined, "a", undefined];
undefinedSort.sort();

let stableSort = [
    { key: 1, order: 0 },
    { key: 1, order: 1 },
    { key: 0, order: 2 },
];
stableSort.sort((a, b) => a.key - b.key);

let spliceTarget = [1, 2, 3, 4, 5];
let spliceRemoved = spliceTarget.splice(1, 2, "a", "b", "c");

let spliceGrow = [1, 2, 3];
spliceGrow.splice(1, 0, "x");

let spliceNegative = [1, 2, 3, 4];
let spliceNegativeRemoved = spliceNegative.splice(-2);

let filled = [1, 2, 3, 4].fill(0, 1, 3);
let copied = [1, 2, 3, 4, 5].copyWithin(0, 3);
let copiedOverlap = [1, 2, 3, 4, 5].copyWithin(1, 3);

let source = [10, 20, 30];
let sorted = [3, 1, 2].toSorted();
let reversed = [1, 2, 3].toReversed();
let spliced = [1, 2, 3, 4].toSpliced(1, 2, "x");
let replaced = [1, 2, 3].with(1, 9);

let rangeError = false;
try {
    [1, 2, 3].with(5, 9);
} catch (error) {
    rangeError = error instanceof RangeError;
}

numericSort.join("|") === "1|2|3|10" &&
    defaultSort.join("|") === "1|10|2|3" &&
    undefinedSort.length === 4 &&
    undefinedSort[0] === "a" &&
    undefinedSort[1] === "b" &&
    undefinedSort[2] === undefined &&
    undefinedSort[3] === undefined &&
    stableSort[0].order === 2 &&
    stableSort[1].order === 0 &&
    stableSort[2].order === 1 &&
    spliceTarget.join("|") === "1|a|b|c|4|5" &&
    spliceRemoved.join("|") === "2|3" &&
    spliceGrow.join("|") === "1|x|2|3" &&
    spliceNegative.join("|") === "1|2" &&
    spliceNegativeRemoved.join("|") === "3|4" &&
    filled.join("|") === "1|0|0|4" &&
    copied.join("|") === "4|5|3|4|5" &&
    copiedOverlap.join("|") === "1|4|5|4|5" &&
    source.at(-1) === 30 &&
    source.at(0) === 10 &&
    source.at(5) === undefined &&
    [1, 2, 3, 4].findLast((value) => value % 2 === 1) === 3 &&
    [1, 2, 3, 4].findLastIndex((value) => value % 2 === 1) === 2 &&
    sorted.join("|") === "1|2|3" &&
    reversed.join("|") === "3|2|1" &&
    spliced.join("|") === "1|x|4" &&
    replaced.join("|") === "1|9|3" &&
    rangeError &&
    Array.prototype.sort.length === 1 &&
    Array.prototype.splice.length === 2 &&
    Array.prototype.fill.length === 1 &&
    Array.prototype.copyWithin.length === 2 &&
    Array.prototype.at.length === 1 &&
    Array.prototype.findLast.length === 1 &&
    Array.prototype.toSorted.length === 1 &&
    Array.prototype.toReversed.length === 0 &&
    Array.prototype.toSpliced.length === 2 &&
    Array.prototype.with.length === 2 &&
    Array.prototype.sort.name === "sort" &&
    Array.prototype.toSpliced.name === "toSpliced"
    ? 42
    : 0
