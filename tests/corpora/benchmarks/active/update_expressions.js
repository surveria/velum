let total = 0;
let record = { value: 0 };
let values = [1, 2, 3, 4];

for (let index = 0; index < 12288; index++) {
    total++;
    record.value++;
    ++values[index & 3];
    if ((index & 7) === 0) {
        --record.value;
    }
}

total + record.value + values[0] + values[1] + values[2] + values[3]
