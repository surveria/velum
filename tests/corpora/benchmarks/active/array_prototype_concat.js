let total = 0;

for (let index = 0; index < 128; index++) {
    let values = [index, index + 1];
    let tail = [index + 2, index + 3];
    let object = { marker: index };
    let result = values.concat(tail, index + 4, object);
    total += result[0];
    total += result[4];
    if (result[5] === object) {
        total += 1;
    }

    let sparse = Array(4);
    sparse[1] = index;
    sparse[3] = index + 1;
    let sparseResult = [0].concat(sparse);
    if (
        sparseResult.length === 5 &&
        !("1" in sparseResult) &&
        sparseResult[2] === index &&
        !("3" in sparseResult) &&
        sparseResult[4] === index + 1
    ) {
        total += 1;
    }
}

total
