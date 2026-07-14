let mathObject = Math;
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
if (
  mathObject !== Math ||
  typeof Math !== "object" ||
  Math.__proto__ !== Object.prototype ||
  typeof Math.abs !== "function" ||
  Math.abs.name !== "abs" ||
  Math.abs.length !== 1 ||
  Math.max.length !== 2 ||
  Math.pow.length !== 2 ||
  !(Math.PI > 3.14) ||
  !(Math.E > 2.71) ||
  !(Math.LN10 > 2.30) ||
  !(Math.LN2 > 0.69) ||
  !(Math.LOG10E > 0.43) ||
  !(Math.LOG2E > 1.44) ||
  !(Math.SQRT1_2 > 0.70) ||
  !(Math.SQRT2 > 1.41) ||
  Math.abs(-7) !== 7 ||
  Math.ceil(1.2) !== 2 ||
  Math.floor(1.8) !== 1 ||
  Math.trunc(-1.8) !== -1 ||
  Math.round(1.5) !== 2 ||
  Math.round(-1.5) !== -1 ||
  Math.sqrt(81) !== 9 ||
  Math.pow(2, 5) !== 32 ||
  Math.max(1, 7, 3) !== 7 ||
  Math.min(1, -2, 3) !== -2 ||
  Math.max() !== -Infinity ||
  Math.min() !== Infinity ||
  nanAbs === nanAbs ||
  maxNaN === maxNaN ||
  minNaN === minNaN ||
  maxPositiveZero !== Infinity ||
  minNegativeZero !== -Infinity ||
  keys !== "" ||
  shadow !== 42 ||
  delete Math !== true
) {
  throw new Test262Error("Math object behavior was unexpected");
}

42
