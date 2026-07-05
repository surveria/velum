let total = 0;

for (let index = 0; index < 128; index++) {
    total += Number();
    total += Number(null);
    total += Number(true);
    total += Number(false);
    total += Number(" 42 ");
    total += Number("1e2");
    total += Number("0x10");
    total += Number("0b101");
    total += Number("0o10");

    let value = new Number(index);
    if (value.__proto__ === Number.prototype) {
        total += 1;
    }
    if (value.constructor === Number) {
        total += 1;
    }
}

if (Number.NaN !== Number.NaN) {
    total += 1;
}
if (Number.POSITIVE_INFINITY > Number.MAX_VALUE) {
    total += 1;
}
if (Number.NEGATIVE_INFINITY < -Number.MAX_VALUE) {
    total += 1;
}

total
