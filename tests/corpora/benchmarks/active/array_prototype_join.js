let total = 0;

for (let index = 0; index < 16384; index++) {
    let values = [index, "x", null, undefined, index + 1];
    if (values.join("|") === index + "|x|||" + (index + 1)) {
        total += 1;
    }

    let sparse = Array(3);
    sparse[1] = "m";
    if (sparse.join(",") === ",m,") {
        total += 1;
    }
}

total
