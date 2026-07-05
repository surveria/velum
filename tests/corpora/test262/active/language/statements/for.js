let values = [10, 20, 10, 2];
let total = 0;

for (let index = 0; index < values.length; index = index + 1) {
    total = total + values[index];
}

if (total !== 42) {
    throw new Test262Error("for statement did not evaluate initializer, condition, update, and body");
}

let updated = 0;
for (let updateIndex = 0; updateIndex < 4; updateIndex = updateIndex + 1) {
    if (updateIndex === 1) {
        continue;
    }
    updated = updated + updateIndex;
}

if (updated !== 5) {
    throw new Test262Error("continue did not evaluate the for update expression");
}

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

if (probe !== 5 || skipped !== 1) {
    throw new Test262Error("break and continue did not control for execution");
}

for (var hoisted = 42; false;) {
    hoisted = 0;
}

if (hoisted !== 42) {
    throw new Test262Error("var initializer did not run before for condition");
}

42
