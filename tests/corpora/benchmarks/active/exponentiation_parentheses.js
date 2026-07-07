let total = 0;
let value = 2;
let target = { slot: 2 };

for (let index = 0; index < 65536; index++) {
    total += (2 ** (index & 3));
    value = (value + 1) & 7;
    (target.slot) += value;
    if ((index & 7) === 0) {
        (target.slot) **= 2;
        target.slot %= 257;
    }
}

total + value + target.slot
