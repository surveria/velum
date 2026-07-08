let keys = "";
for (let key in Number) {
  keys = keys + key + ";";
}

if (
  typeof Number.isNaN !== "function" ||
  Number.isNaN.name !== "isNaN" ||
  Number.isNaN.length !== 1 ||
  typeof Number.isFinite !== "function" ||
  Number.isFinite.name !== "isFinite" ||
  Number.isFinite.length !== 1 ||
  Number.parseInt !== parseInt ||
  Number.parseFloat !== parseFloat ||
  Number.isNaN(NaN) !== true ||
  Number.isNaN("NaN") !== false ||
  Number.isNaN(undefined) !== false ||
  Number.isFinite(42) !== true ||
  Number.isFinite("42") !== false ||
  Number.isFinite(Infinity) !== false ||
  keys !== ""
) {
  throw new Test262Error("Number static method behavior was unexpected");
}

42
