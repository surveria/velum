let values = [1, 2, 3, 4];
let total = 0;

for (let index = 0; index < 24576; index = index + 1) {
    if ((index & 3) === 0) {
        continue;
    }
    total = total + values[index & 3];
}

total;
