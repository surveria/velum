let total = 0;
let index = 0;

while (index < 65536) {
  if (NaN !== NaN) {
    total = total + 1;
  }
  if (Infinity > 1e300) {
    total = total + 1;
  }
  if (-Infinity < -1e300) {
    total = total + 1;
  }
  if (typeof NaN === "number") {
    total = total + 1;
  }
  index = index + 1;
}

total
