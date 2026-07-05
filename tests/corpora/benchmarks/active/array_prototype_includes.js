let total = 0;

for (let index = 0; index < 128; index++) {
    let values = [index, index + 1, index + 2, index + 1];
    if (values.includes(index + 1)) {
        total += 1;
    }
    if (values.includes(index + 1, 2)) {
        total += 1;
    }
    if (!values.includes(-1)) {
        total += 1;
    }

    let sparse = Array(4);
    sparse[3] = index;
    if (sparse.includes(index)) {
        total += 1;
    }
    if (sparse.includes(undefined)) {
        total += 1;
    }
}

total
