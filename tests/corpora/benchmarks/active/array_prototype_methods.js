let total = 0;

for (let index = 0; index < 128; index++) {
    let values = [];
    total += values.push(index);
    total += values.push(index + 1, index + 2);
    total += values.pop();
    total += values.pop();
    total += values.pop();
    total += values.length;
}

total
