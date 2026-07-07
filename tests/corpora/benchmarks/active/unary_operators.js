let total = 0;

for (let index = 0; index < 4096; index = index + 1) {
    let record = { value: index, stale: 1 };
    if (typeof record.value === "number") {
        total = total + record.value;
    }
    if (delete record.stale) {
        total = total + 1;
    }
    void (record.extra = index & 3);
}

total
