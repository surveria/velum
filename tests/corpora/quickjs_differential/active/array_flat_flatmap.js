let nested = [1, [2, [3, [4]]], 5];
let flatDefault = nested.flat();
let flatTwo = nested.flat(2);

let sparse = Array(5);
sparse[0] = [1, 2];
sparse[2] = [3, [4]];
sparse[4] = 5;
let flatSparse = sparse.flat(2);

let values = [1, 2, 3];
let thisArg = { scale: 10 };
let mapped = values.flatMap(function(value, index, receiver) {
    return [value * this.scale, index, receiver === values];
}, thisArg);

let object = { 0: [1, [2]], 2: [3], length: 3 };
let genericFlat = Array.prototype.flat.call(object, 2);
let genericFlatMap = Array.prototype.flatMap.call(object, function(value, index) {
    return [index, value];
});

let missingCallback = false;
try {
    values.flatMap();
} catch (error) {
    missingCallback = true;
}

print(
    "flat",
    flatDefault.length,
    flatDefault[0],
    flatDefault[1],
    flatDefault[2][0],
    flatDefault[2][1][0],
    flatDefault[3],
    flatTwo.length,
    flatTwo[0],
    flatTwo[1],
    flatTwo[2],
    flatTwo[3][0],
    flatTwo[4],
    flatSparse.length,
    flatSparse.join("|")
);
print("flatMap", mapped.join("|"));
print(
    "generic",
    genericFlat.join("|"),
    genericFlatMap.length,
    genericFlatMap[0],
    genericFlatMap[1][0],
    genericFlatMap[1][1][0],
    genericFlatMap[2],
    genericFlatMap[3][0]
);
print("errors", missingCallback);
