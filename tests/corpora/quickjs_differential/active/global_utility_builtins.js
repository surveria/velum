let parseIntInvalid = parseInt("2", 2);
let parseFloatInvalid = parseFloat(".");
let invalidPercent = false;
let invalidUtf8 = false;

try {
  decodeURIComponent("%");
} catch (error) {
  invalidPercent = error.name;
}

try {
  decodeURIComponent("%E0%A4%A");
} catch (error) {
  invalidUtf8 = error.name;
}

print(parseInt.name, parseInt.length, parseFloat.name, parseFloat.length);
print(parseInt("  -0xF"), parseInt("11", 2), parseInt("12px", 10), parseInt("08"), parseIntInvalid);
print(parseFloat("  -1.25e2px"), parseFloat(".5x"), parseFloat("1.e2px"), parseFloat("Infinity!"), parseFloatInvalid);
print(isNaN("not-a-number"), isNaN("42"), isFinite("42"), isFinite("not-a-number"));
print(Number.isNaN(NaN), Number.isNaN("NaN"), Number.isFinite(42), Number.isFinite("42"));
print(Number.parseInt === parseInt, Number.parseFloat === parseFloat);
print(encodeURI("front camera?x=1&name=камера"));
print(encodeURIComponent("front camera?x=1&name=камера"));
print(decodeURI("front%20camera%3Fx=1%26name=%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0"));
print(decodeURIComponent("front%20camera%3Fx%3D1%26name%3D%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0"));
print(invalidPercent, invalidUtf8);
