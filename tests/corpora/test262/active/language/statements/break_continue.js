let values = [20, 1, 22, 100];
let index = 0;
let total = 0;

while (index < values.length) {
    if (index === 1) {
        index = index + 1;
        continue;
    }
    if (index === 3) {
        break;
    }
    total = total + values[index];
    index = index + 1;
}

if (index !== 3 || total !== 42) {
    throw new Test262Error("break and continue did not control while execution");
}

let probe = 0;
while (probe < 3) {
    probe = probe + 1;
    try {
        continue;
    } catch (error) {
        throw new Test262Error("continue was caught as an exception");
    }
}

if (probe !== 3) {
    throw new Test262Error("continue did not propagate through try");
}

42
