// Coverage for Number.prototype numeric-formatting methods and the ECMAScript
// Number ToString notation. Evaluates to 42 on full spec conformance, else 0.

let fixedRange = 0;
try {
    (1).toFixed(-1);
} catch (error) {
    if (error instanceof RangeError) fixedRange += 1;
}
try {
    (1).toFixed(101);
} catch (error) {
    if (error instanceof RangeError) fixedRange += 1;
}

let precisionRange = 0;
try {
    (1).toPrecision(0);
} catch (error) {
    if (error instanceof RangeError) precisionRange += 1;
}
try {
    (1).toPrecision(101);
} catch (error) {
    if (error instanceof RangeError) precisionRange += 1;
}

(0).toFixed(0) === "0" &&
    (1).toFixed(0) === "1" &&
    (2.5).toFixed(0) === "3" &&
    (0.5).toFixed(0) === "1" &&
    (1.005).toFixed(2) === "1.00" &&
    (123.456).toFixed(2) === "123.46" &&
    (255).toFixed(0) === "255" &&
    (1.1).toFixed(1) === "1.1" &&
    (-1.23).toFixed(1) === "-1.2" &&
    (0).toFixed(2) === "0.00" &&
    (1e21).toFixed(2) === "1e+21" &&
    (NaN).toFixed(2) === "NaN" &&
    (123.456).toExponential(2) === "1.23e+2" &&
    (0.0001234).toExponential(2) === "1.23e-4" &&
    (5).toExponential(0) === "5e+0" &&
    (123.456).toExponential() === "1.23456e+2" &&
    (0).toExponential() === "0e+0" &&
    (123.456).toPrecision(4) === "123.5" &&
    (0.0001234).toPrecision(2) === "0.00012" &&
    (123456).toPrecision(3) === "1.23e+5" &&
    (1.5).toPrecision(1) === "2" &&
    (100).toPrecision(5) === "100.00" &&
    (123.456).toPrecision() === "123.456" &&
    String(1e21) === "1e+21" &&
    String(1e-7) === "1e-7" &&
    String(0.00001) === "0.00001" &&
    String(1e20) === "100000000000000000000" &&
    Number.MIN_VALUE === 5e-324 &&
    fixedRange === 2 &&
    precisionRange === 2 &&
    Number.prototype.toFixed.length === 1 &&
    Number.prototype.toExponential.length === 1 &&
    Number.prototype.toPrecision.length === 1 &&
    Number.prototype.toFixed.name === "toFixed" &&
    Number.prototype.toExponential.name === "toExponential" &&
    Number.prototype.toPrecision.name === "toPrecision"
    ? 42
    : 0
