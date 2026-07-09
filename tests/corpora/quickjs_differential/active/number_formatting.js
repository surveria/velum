var fixed = [
    [0, 0], [1, 0], [1.5, 0], [2.5, 0], [0.5, 0], [-0.5, 0], [-2.5, 0],
    [1.005, 2], [8.575, 2], [123.456, 2], [0, 2], [1.1, 1], [255, 0],
    [0.00001, 2], [-1.23, 1], [3.14159, 4], [1234.5678, 2], [9.999, 2],
    [0.1, 1], [2.55, 1], [99.995, 2], [1000000, 0]
];
for (var i = 0; i < fixed.length; i++) {
    print("F", fixed[i][0], fixed[i][1], fixed[i][0].toFixed(fixed[i][1]));
}

var exp = [
    [123.456, 2], [0.0001234, 2], [5, 0], [100, 2], [0.5, 3], [-9.9, 1],
    [1234567, 3], [0, 4], [77.1234, 5], [1, 0], [999.9, 1]
];
for (var i = 0; i < exp.length; i++) {
    print("E", exp[i][0], exp[i][1], exp[i][0].toExponential(exp[i][1]));
}
print("Eu", (123.456).toExponential(), (100).toExponential(), (0).toExponential(), (0.00007).toExponential());

var prec = [
    [123.456, 4], [0.0001234, 2], [123456, 3], [1.5, 1], [0.00001, 3],
    [100, 5], [1234.5, 2], [9.99, 2], [0, 3], [45.6, 6], [1000, 2]
];
for (var i = 0; i < prec.length; i++) {
    print("P", prec[i][0], prec[i][1], prec[i][0].toPrecision(prec[i][1]));
}
print("Pu", (5.5).toPrecision(), (12345).toPrecision());

var strings = [
    0, 1, -1, 100, 0.5, 0.1, 0.001, 1e20, 1e21, 1e-6, 1e-7, 123456789,
    5e-324, 9007199254740991, -0.0000001, 12345.6789, 1e100, -1e21,
    0.30000000000000004, 255, 100000000000000000000, 1.7976931348623157e308
];
for (var i = 0; i < strings.length; i++) {
    print("S", "" + strings[i]);
}

print("MIN", Number.MIN_VALUE, Number.MIN_VALUE > 0, Number.MIN_VALUE / 2 === 0);

var errors = "";
try { (1).toFixed(-1); } catch (e) { errors += e.constructor.name + ";"; }
try { (1).toFixed(101); } catch (e) { errors += e.constructor.name + ";"; }
try { (1).toExponential(101); } catch (e) { errors += e.constructor.name + ";"; }
try { (1).toPrecision(0); } catch (e) { errors += e.constructor.name + ";"; }
print("errors", errors);

print("meta", Number.prototype.toFixed.length, Number.prototype.toExponential.name, Number.prototype.toPrecision.name);
