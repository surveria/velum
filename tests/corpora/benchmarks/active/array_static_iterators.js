let seed = [];
for (let index = 0; index < 128; index = index + 1) {
    seed.push(index);
}

function mapValue(value, index) {
    return value + index;
}

let total = 0;
for (let round = 0; round < 128; round = round + 1) {
    let fromArray = Array.from(seed, mapValue);
    let fromObject = Array.from({ 0: round, 1: round + 1, 2: round + 2, length: 3 });
    let ofArray = Array.of(round, round + 1, round + 2, round + 3);
    let iterator = fromArray.values();

    total = total + fromArray.length + fromObject.length + ofArray.length;
    for (let index = 0; index < 128; index = index + 1) {
        total = total + iterator.next().value;
    }
    let entries = ofArray.entries();
    total = total + entries.next().value[1];
    total = total + entries.next().value[1];
}

total
