let total = 0;
let object = { a: 1, b: 2, empty: undefined };
let values = [1, 2, 3, 4];

for (let index = 0; index < 65536; index++) {
    if ("a" in object) {
        total += object.a;
    }
    if ("empty" in object) {
        total += 1;
    }
    if ((index & 3) in values) {
        total += 1;
    }
    if ("missing" in object) {
        total += 64;
    }
}

total
