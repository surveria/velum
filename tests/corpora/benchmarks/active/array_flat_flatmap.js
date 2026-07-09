let nested = [];
for (let index = 0; index < 32; index = index + 1) {
    nested.push([index, [index + 1, [index + 2]]]);
}

function expand(value, index) {
    return [value, index, value + index];
}

let total = 0;
for (let round = 0; round < 80; round = round + 1) {
    let flattened = nested.flat(3);
    let mapped = flattened.flatMap(expand);
    total = total + flattened.length + mapped.length;
    for (let position = 0; position < mapped.length; position = position + 2) {
        total = total + mapped[position];
    }
    total = total + mapped[round % mapped.length];
}

total
