let shadow = 0;
{
  let NaN = 40;
  let Infinity = 2;
  shadow = NaN + Infinity;
}

print(typeof NaN, NaN !== NaN, Infinity > 1e300, -Infinity < -1e300, shadow);
