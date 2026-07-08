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

flatDefault.length === 4 &&
    flatDefault[0] === 1 &&
    flatDefault[1] === 2 &&
    flatDefault[2][0] === 3 &&
    flatDefault[2][1][0] === 4 &&
    flatDefault[3] === 5 &&
    flatTwo.length === 5 &&
    flatTwo[0] === 1 &&
    flatTwo[1] === 2 &&
    flatTwo[2] === 3 &&
    flatTwo[3][0] === 4 &&
    flatTwo[4] === 5 &&
    flatSparse.length === 5 &&
    flatSparse.join("|") === "1|2|3|4|5" &&
    mapped.join("|") === "10|0|true|20|1|true|30|2|true" &&
    genericFlat.join("|") === "1|2|3" &&
    genericFlatMap.length === 4 &&
    genericFlatMap[0] === 0 &&
    genericFlatMap[1][0] === 1 &&
    genericFlatMap[1][1][0] === 2 &&
    genericFlatMap[2] === 2 &&
    genericFlatMap[3][0] === 3 &&
    missingCallback ? 42 : 0
