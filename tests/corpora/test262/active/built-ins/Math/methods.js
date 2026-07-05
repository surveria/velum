let near = function(actual, expected) {
  return Math.abs(actual - expected) < 0.000001;
};

let metadataOk =
  Math.acos.name === "acos" &&
  Math.acos.length === 1 &&
  Math.atan2.name === "atan2" &&
  Math.atan2.length === 2 &&
  Math.hypot.name === "hypot" &&
  Math.hypot.length === 2;

let trigOk =
  near(Math.acos(1), 0) &&
  near(Math.asin(0), 0) &&
  near(Math.atan(1), Math.PI / 4) &&
  near(Math.atan2(1, 1), Math.PI / 4) &&
  near(Math.cos(0), 1) &&
  near(Math.sin(0), 0) &&
  near(Math.tan(0), 0);

let logOk =
  near(Math.exp(1), Math.E) &&
  near(Math.expm1(0), 0) &&
  near(Math.log(Math.E), 1) &&
  near(Math.log10(100), 2) &&
  near(Math.log1p(0), 0) &&
  near(Math.log2(8), 3);

let rootOk =
  near(Math.cbrt(27), 3) &&
  near(Math.cbrt(-8), -2) &&
  Math.sqrt(81) === 9;

let signOk =
  Math.sign(-2) === -1 &&
  Math.sign(2) === 1 &&
  Math.sign(0) === 0 &&
  1 / Math.sign(-0) === -Infinity;

let hyperOk =
  near(Math.sinh(0), 0) &&
  near(Math.cosh(0), 1) &&
  near(Math.tanh(0), 0) &&
  near(Math.asinh(0), 0) &&
  near(Math.acosh(1), 0) &&
  near(Math.atanh(0), 0);

let hypotOk =
  Math.hypot() === 0 &&
  Math.hypot(3, 4) === 5 &&
  Math.hypot(Infinity, NaN) === Infinity &&
  Math.hypot(NaN, 3) !== Math.hypot(NaN, 3);

let nanOk =
  Math.acos() !== Math.acos() &&
  Math.log(-1) !== Math.log(-1) &&
  Math.sign(NaN) !== Math.sign(NaN);

if (
  !metadataOk ||
  !trigOk ||
  !logOk ||
  !rootOk ||
  !signOk ||
  !hyperOk ||
  !hypotOk ||
  !nanOk
) {
  throw new Test262Error("Math methods behavior was unexpected");
}

42
