let values = [1, 2, 3, 4];
let thisArg = { total: 0 };
let forEachResult = values.forEach(function(value, index, array) {
    this.total = this.total + value + index + (array === values ? 10 : 0);
}, thisArg);

let mapped = values.map(function(value, index) {
    return value * 10 + index;
});
let filtered = values.filter(function(value, index) {
    return value > 2 && index < 4;
});
let some = values.some(function(value) {
    return value === 3;
});
let every = values.every(function(value) {
    return value > 0;
});
let found = values.find(function(value) {
    return value > 2;
});
let foundIndex = values.findIndex(function(value) {
    return value > 2;
});
let reduced = values.reduce(function(acc, value, index) {
    return acc + value + index;
}, 0);
let reducedRight = values.reduceRight(function(acc, value) {
    return acc + "" + value;
}, "");

let sparse = Array(4);
sparse[1] = 2;
sparse[3] = 4;
let sparseVisited = "";
sparse.forEach(function(value, index) {
    sparseVisited = sparseVisited + index + ":" + value + ";";
});
let sparseMapped = sparse.map(function(value, index) {
    return value * 10 + index;
});
let sparseFindVisited = "";
let sparseFound = sparse.find(function(value, index) {
    sparseFindVisited = sparseFindVisited + index + ":" + value + ";";
    return index === 0;
});
let sparseFoundIndex = sparse.findIndex(function(value, index) {
    return index === 2;
});

let object = { length: 3, 0: 1, 2: 3 };
let objectSeen = "";
let objectMapped = Array.prototype.map.call(object, function(value, index, receiver) {
    objectSeen = objectSeen + index + ":" + value + ":" + (receiver === object) + ";";
    return value + 1;
});
let objectReduced = Array.prototype.reduce.call(object, function(acc, value, index) {
    return acc + value + index;
}, 0);

let missingCallback = false;
let emptyReduce = false;
try {
    [1].map();
} catch (error) {
    missingCallback = true;
}
try {
    [].reduce(function(acc, value) {
        return acc + value;
    });
} catch (error) {
    emptyReduce = true;
}

forEachResult === undefined &&
    thisArg.total === 56 &&
    mapped.join("|") === "10|21|32|43" &&
    filtered.join("|") === "3|4" &&
    some === true &&
    every === true &&
    found === 3 &&
    foundIndex === 2 &&
    reduced === 16 &&
    reducedRight === "4321" &&
    sparseVisited === "1:2;3:4;" &&
    sparseMapped.length === 4 &&
    !("0" in sparseMapped) &&
    sparseMapped[1] === 21 &&
    !("2" in sparseMapped) &&
    sparseMapped[3] === 43 &&
    sparseFindVisited === "0:undefined;" &&
    sparseFound === undefined &&
    sparseFoundIndex === 2 &&
    objectSeen === "0:1:true;2:3:true;" &&
    objectMapped.length === 3 &&
    objectMapped[0] === 2 &&
    !("1" in objectMapped) &&
    objectMapped[2] === 4 &&
    objectReduced === 6 &&
    missingCallback &&
    emptyReduce ? 42 : 0
