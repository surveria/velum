// Coverage for the ES2025 Set.prototype set-operation methods. Evaluates to 42
// when every observable behavior matches the specification, otherwise 0.

function set(values) {
    return new Set(values);
}

function join(collection) {
    let result = "";
    collection.forEach(function (value) {
        result = result === "" ? String(value) : result + "," + value;
    });
    return result;
}

let base = set([1, 2, 3]);
let union = base.union(set([3, 4, 5]));
let intersection = set([1, 2, 3, 4]).intersection(set([2, 4, 6]));
let difference = set([1, 2, 3, 4]).difference(set([2, 4]));
let symmetric = set([1, 2, 3]).symmetricDifference(set([2, 3, 4]));

// Set-like argument: a Map exposes size, has, and keys.
let setLike = new Map([
    [2, "b"],
    [3, "c"],
    [9, "z"],
]);
let setLikeIntersection = set([1, 2, 3]).intersection(setLike);

let typeError = false;
try {
    set([1]).union(42);
} catch (error) {
    typeError = error instanceof TypeError;
}

let rangeError = false;
try {
    set([1]).union({ size: -1, has: function () {}, keys: function () {} });
} catch (error) {
    rangeError = error instanceof RangeError;
}

join(union) === "1,2,3,4,5" &&
    join(intersection) === "2,4" &&
    join(difference) === "1,3" &&
    join(symmetric) === "1,4" &&
    join(base) === "1,2,3" &&
    join(setLikeIntersection) === "2,3" &&
    set([1, 2]).isSubsetOf(set([1, 2, 3])) === true &&
    set([1, 2, 3]).isSubsetOf(set([1, 2])) === false &&
    set([1, 2, 3]).isSupersetOf(set([1, 2])) === true &&
    set([1, 2]).isSupersetOf(set([1, 2, 3])) === false &&
    set([1, 2]).isDisjointFrom(set([3, 4])) === true &&
    set([1, 2]).isDisjointFrom(set([2, 3])) === false &&
    typeError &&
    rangeError &&
    Set.prototype.union.length === 1 &&
    Set.prototype.intersection.length === 1 &&
    Set.prototype.difference.length === 1 &&
    Set.prototype.symmetricDifference.length === 1 &&
    Set.prototype.isSubsetOf.length === 1 &&
    Set.prototype.isSupersetOf.length === 1 &&
    Set.prototype.isDisjointFrom.length === 1 &&
    Set.prototype.union.name === "union" &&
    Set.prototype.symmetricDifference.name === "symmetricDifference" &&
    Set.prototype.isDisjointFrom.name === "isDisjointFrom"
    ? 42
    : 0
