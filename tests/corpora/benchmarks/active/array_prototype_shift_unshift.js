let total = 0;

for (let index = 0; index < 128; index++) {
    let values = [index + 1, index + 2];
    total += values.unshift(index);
    if (values.shift() === index) {
        total += 1;
    }
    total += values.length;

    let sparse = Array(3);
    sparse[2] = index;
    total += sparse.unshift(index + 1);
    if (sparse.shift() === index + 1 && !("0" in sparse) && sparse[2] === index) {
        total += 1;
    }
    total += sparse.length;
}

total
