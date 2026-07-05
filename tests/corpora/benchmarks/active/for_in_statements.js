let total = 0;
let object = { alpha: 1, beta: 2, gamma: 3, delta: 4 };
let values = [1, 2, 3, 4];
values[6] = 7;

for (let round = 0; round < 96; round++) {
    for (let key in object) {
        total += object[key];
    }
    for (let index in values) {
        total += values[index];
    }
}

total
