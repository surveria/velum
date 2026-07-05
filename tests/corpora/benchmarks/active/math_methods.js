let total = 0;
let index = 0;

while (index < 200) {
  let value = index + 1;
  total = total + Math.acos(1);
  total = total + Math.asin(0);
  total = total + Math.atan(value);
  total = total + Math.atan2(value, value + 1);
  total = total + Math.cos(value);
  total = total + Math.sin(value);
  total = total + Math.tan(0);
  total = total + Math.exp(1);
  total = total + Math.expm1(0);
  total = total + Math.log(value);
  total = total + Math.log10(value);
  total = total + Math.log1p(value);
  total = total + Math.log2(value);
  total = total + Math.cbrt(value);
  total = total + Math.sign(value - 100);
  total = total + Math.sinh(0);
  total = total + Math.cosh(0);
  total = total + Math.tanh(0);
  total = total + Math.asinh(0);
  total = total + Math.acosh(1 + (index % 5));
  total = total + Math.atanh(0);
  total = total + Math.hypot(value, 3);
  index = index + 1;
}

total
