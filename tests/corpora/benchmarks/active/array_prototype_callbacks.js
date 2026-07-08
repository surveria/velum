let seed = [];
for (let index = 0; index < 128; index = index + 1) {
    seed.push(index);
}

function mapValue(value, index) {
    return value + index;
}

function keepValue(value) {
    return value % 3 === 0;
}

function sumValues(acc, value) {
    return acc + value;
}

function findNeedle(value) {
    return value === 126;
}

let total = 0;
for (let round = 0; round < 128; round = round + 1) {
    let mapped = seed.map(mapValue);
    let filtered = mapped.filter(keepValue);
    total = total + filtered.reduce(sumValues, 0);
    total = total + seed.reduceRight(sumValues, 0);
    total = total + (seed.some(findNeedle) ? 1 : 0);
    total = total + (seed.every(function(value) { return value >= 0; }) ? 1 : 0);
    total = total + seed.findIndex(findNeedle);
}

total
