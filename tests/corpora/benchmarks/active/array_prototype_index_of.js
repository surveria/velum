let total = 0;

for (let index = 0; index < 128; index++) {
    let values = [index, index + 1, index + 2, index + 1];
    total += values.indexOf(index + 1);
    total += values.indexOf(index + 1, 2);
    total += values.indexOf(-1);

    let sparse = Array(4);
    sparse[3] = index;
    total += sparse.indexOf(index);
    total += sparse.indexOf(undefined);
}

total
