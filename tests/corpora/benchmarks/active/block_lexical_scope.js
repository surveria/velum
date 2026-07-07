let total = 0;

for (let outer = 0; outer < 8192; outer = outer + 1) {
    let record = { value: outer & 3 };
    {
        let inner = record.value + 1;
        const bump = inner + outer;
        total = total + (bump & 7);
    }
}

total
