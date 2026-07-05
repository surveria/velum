let keys = "";
for (let key in Math) {
    keys = keys + key + ";";
}

let shadow = 0;
{
    let Math = {
        abs: function(value) {
            return value + 35;
        }
    };
    shadow = Math.abs(7);
}

let nanAbs = Math.abs();
let maxNaN = Math.max(1, NaN);
let minNaN = Math.min(NaN, 1);
let maxPositiveZero = 1 / Math.max(-0, 0);
let minNegativeZero = 1 / Math.min(0, -0);

print(
    typeof Math,
    Math.__proto__ === Object.prototype,
    Math.PI > 3.14,
    Math.E > 2.71,
    Math.abs.name,
    Math.max.length,
    Math.pow.length
);
print(
    Math.abs(-7),
    Math.ceil(1.2),
    Math.floor(1.8),
    Math.trunc(-1.8),
    Math.round(1.5),
    Math.round(-1.5),
    Math.sqrt(81),
    Math.pow(2, 5),
    Math.max(1, 7, 3),
    Math.min(1, -2, 3)
);
print(
    Math.max(),
    Math.min(),
    nanAbs !== nanAbs,
    maxNaN !== maxNaN,
    minNaN !== minNaN
);
print(maxPositiveZero === Infinity, minNegativeZero === -Infinity, "keys:" + keys, shadow);
