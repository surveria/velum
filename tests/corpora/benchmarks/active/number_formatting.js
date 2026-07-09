let total = 0;

for (let round = 0; round < 2000; round++) {
    let base = (round + 1) * 0.123456789;
    for (let digits = 0; digits < 8; digits++) {
        total += base.toFixed(digits).length;
        total += base.toExponential(digits).length;
        total += base.toPrecision(digits + 1).length;
    }
    total += ("" + (base * 1e18)).length;
    total += ("" + (base * 1e-12)).length;
}

total
