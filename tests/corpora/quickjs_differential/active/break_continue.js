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

print(index);
print(total);

let probe = 0;
while (probe < 3) {
    probe = probe + 1;
    try {
        continue;
    } catch (error) {
        print("caught");
    }
}
print(probe);
