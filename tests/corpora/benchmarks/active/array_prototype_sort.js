let total = 0;

for (let round = 0; round < 256; round++) {
    let values = [];
    let seed = round + 1;
    for (let index = 0; index < 96; index++) {
        seed = (seed * 1103515245 + 12345) % 2147483648;
        values[index] = seed % 1000;
    }

    values.sort((a, b) => a - b);
    total += values[0];
    total += values[95];

    let ascending = values.sort((a, b) => a - b);
    if (ascending[0] <= ascending[95]) {
        total += 1;
    }

    let copy = values.toSorted((a, b) => b - a);
    total += copy[0];

    let spliced = values.toSpliced(10, 20, -1, -2, -3);
    total += spliced.length;

    let reversed = values.toReversed();
    total += reversed[0];

    values.splice(0, 32);
    total += values.length;
}

total
