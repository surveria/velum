let total = 0;

for (let index = 0; index < 4096; index++) {
    let values = [index, index + 1, index + 2, index + 3];
    let middle = values.slice(1, 3);
    total += middle.length;
    total += middle[0];
    total += middle[1];

    let sparse = Array(4);
    sparse[3] = index;
    let sparseCopy = sparse.slice(1);
    if (!("0" in sparseCopy) && sparseCopy[2] === index) {
        total += 1;
    }
    total += sparseCopy.length;
}

total
