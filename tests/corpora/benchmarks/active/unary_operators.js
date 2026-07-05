let total = 0;
let record = {};

for (let index = 0; index < 96; index = index + 1) {
    record = { value: index, stale: 1 };
    if (typeof record.value === "number") {
        total = total + record.value;
    }
    if (delete record.stale) {
        total = total + 1;
    }
    void (record.extra = index & 3);
}

total
