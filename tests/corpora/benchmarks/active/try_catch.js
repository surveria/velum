var value = 0;
try {
  throw "caught";
  value = 100;
} catch (error) {
  value = value + 1;
}
undefined;
