let total = 0;

for (let index = 0; index < 4096; index++) {
    let values = [index, index + 1, index + 2, index + 3];
    values.reverse();
    total += values[0];
    total += values[3];

    let sparse = Array(4);
    sparse[1] = index;
    sparse[3] = index + 1;
    sparse.reverse();
    if (sparse[0] === index + 1 && sparse[2] === index && !("1" in sparse) && !("3" in sparse)) {
        total += 1;
    }
}

total
