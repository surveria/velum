let values = [10, 20, 10, 2];
let total = 0;

for (let index = 0; index < values.length; index = index + 1) {
    total = total + values[index];
}

print(total);

let updated = 0;
for (let updateIndex = 0; updateIndex < 4; updateIndex = updateIndex + 1) {
    if (updateIndex === 1) {
        continue;
    }
    updated = updated + updateIndex;
}

print(updated);

let probe = 0;
let skipped = 0;
for (;;) {
    probe = probe + 1;
    if (probe === 2) {
        skipped = skipped + 1;
        continue;
    }
    if (probe === 5) {
        break;
    }
}

print(probe);
print(skipped);

for (var hoisted = 42; false;) {
    hoisted = 0;
}

print(hoisted);
