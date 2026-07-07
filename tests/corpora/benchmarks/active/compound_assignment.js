let total = 0;
let record = { count: 1 };
let values = [1, 2, 3, 4];

for (let index = 0; index < 65536; index++) {
    total += index & 3;
    record.count += 2;
    values[index & 3] += record.count & 1;
    if ((index & 7) === 0) {
        record.count -= 1;
    }
}

total + record.count + values[0] + values[1] + values[2] + values[3]
