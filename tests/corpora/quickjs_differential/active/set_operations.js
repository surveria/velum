function set(values) {
    return new Set(values);
}

function join(collection) {
    let parts = [];
    collection.forEach(function (value) {
        parts.push(value);
    });
    return parts.join("|");
}

print("union", join(set([1, 2, 3]).union(set([3, 4, 5]))));
print("union-empty", join(set([]).union(set([1, 2]))));
print("intersection-small", join(set([1, 2, 3]).intersection(set([2, 3, 4, 5, 6]))));
print("intersection-large", join(set([1, 2, 3, 4, 5, 6]).intersection(set([2, 4]))));
print("difference-small", join(set([1, 2, 3]).difference(set([2, 3, 4, 5, 6]))));
print("difference-large", join(set([1, 2, 3, 4, 5, 6]).difference(set([2, 4]))));
print("symmetric", join(set([1, 2, 3]).symmetricDifference(set([3, 4, 5]))));

print("subset", set([1, 2]).isSubsetOf(set([1, 2, 3])), set([1, 2, 3]).isSubsetOf(set([1, 2])));
print("superset", set([1, 2, 3]).isSupersetOf(set([2, 3])), set([1, 2]).isSupersetOf(set([1, 2, 3])));
print("disjoint-small", set([1, 2]).isDisjointFrom(set([3, 4, 5, 6])), set([1, 2]).isDisjointFrom(set([2, 3, 4, 5])));
print("disjoint-large", set([1, 2, 3, 4]).isDisjointFrom(set([5, 6])), set([1, 2, 3, 4]).isDisjointFrom(set([4, 5])));

// Set-like argument backed by a Map (size, has, keys).
let setLike = new Map([
    [2, "b"],
    [4, "d"],
    [6, "f"],
]);
print("setlike-union", join(set([1, 2, 3]).union(setLike)));
print("setlike-intersection", join(set([1, 2, 3, 4, 5, 6, 7]).intersection(setLike)));
print("setlike-difference", join(set([1, 2, 3, 4, 5, 6, 7]).difference(setLike)));
print("setlike-superset", set([1, 2, 3, 4, 5, 6]).isSupersetOf(setLike));

// Zero normalization: -0 and +0 collapse.
print("zero", join(set([-0]).union(set([0]))), set([0]).has(-0));

// Ordering: receiver order preserved, then argument order.
print("order", join(set([3, 1, 2]).union(set([5, 4, 2]))));

// Set-like whose keys method returns a real Set iterator (deduplicated).
let custom = {
    size: 2,
    has: function (value) {
        return value === 2 || value === 9;
    },
    keys: function () {
        return new Set([9, 9, 2]).keys();
    },
};
print("custom-union", join(set([1, 2]).union(custom)));
print("custom-superset", set([1, 2, 9]).isSupersetOf(custom));

let errors = "";
try {
    set([1]).union(42);
} catch (error) {
    errors += error.constructor.name + ";";
}
try {
    set([1]).union({ size: NaN, has: function () {}, keys: function () {} });
} catch (error) {
    errors += error.constructor.name + ";";
}
try {
    set([1]).union({ size: -1, has: function () {}, keys: function () {} });
} catch (error) {
    errors += error.constructor.name + ";";
}
try {
    set([1]).union({ size: 1, has: 5, keys: function () {} });
} catch (error) {
    errors += error.constructor.name + ";";
}
print("errors", errors);

print(
    "meta",
    Set.prototype.union.length,
    Set.prototype.intersection.name,
    Set.prototype.isDisjointFrom.name
);
