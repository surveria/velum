let total = 0;

for (let round = 0; round < 400; round++) {
    let left = new Set();
    let right = new Set();
    for (let index = 0; index < 64; index++) {
        left.add((index + round) % 96);
        right.add((index * 2 + round) % 96);
    }

    let union = left.union(right);
    total += union.size;

    let intersection = left.intersection(right);
    total += intersection.size;

    let difference = left.difference(right);
    total += difference.size;

    let symmetric = left.symmetricDifference(right);
    total += symmetric.size;

    if (left.isSubsetOf(union)) {
        total += 1;
    }
    if (union.isSupersetOf(left)) {
        total += 1;
    }
    if (left.isDisjointFrom(difference) === false) {
        total += 1;
    }
}

total
