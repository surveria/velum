let parseIntInvalid = parseInt("2", 2);
let parseFloatInvalid = parseFloat(".");
let invalidPercent = false;
let invalidUtf8 = false;

try {
  decodeURIComponent("%");
} catch (error) {
  invalidPercent = error.name === "URIError";
}

try {
  decodeURIComponent("%E0%A4%A");
} catch (error) {
  invalidUtf8 = error.name === "URIError";
}

if (
  typeof parseInt !== "function" ||
  parseInt.name !== "parseInt" ||
  parseInt.length !== 2 ||
  typeof parseFloat !== "function" ||
  parseFloat.name !== "parseFloat" ||
  parseFloat.length !== 1 ||
  typeof isNaN !== "function" ||
  isNaN.name !== "isNaN" ||
  isNaN.length !== 1 ||
  typeof isFinite !== "function" ||
  isFinite.name !== "isFinite" ||
  isFinite.length !== 1 ||
  typeof encodeURI !== "function" ||
  encodeURI.name !== "encodeURI" ||
  encodeURI.length !== 1 ||
  typeof encodeURIComponent !== "function" ||
  encodeURIComponent.name !== "encodeURIComponent" ||
  encodeURIComponent.length !== 1 ||
  typeof decodeURI !== "function" ||
  decodeURI.name !== "decodeURI" ||
  decodeURI.length !== 1 ||
  typeof decodeURIComponent !== "function" ||
  decodeURIComponent.name !== "decodeURIComponent" ||
  decodeURIComponent.length !== 1 ||
  parseInt("  -0xF") !== -15 ||
  parseInt("11", 2) !== 3 ||
  parseInt("12px", 10) !== 12 ||
  parseInt("08") !== 8 ||
  parseIntInvalid === parseIntInvalid ||
  parseFloat("  -1.25e2px") !== -125 ||
  parseFloat(".5x") !== 0.5 ||
  parseFloat("1.e2px") !== 100 ||
  parseFloat("Infinity!") !== Infinity ||
  parseFloatInvalid === parseFloatInvalid ||
  isNaN("not-a-number") !== true ||
  isNaN("42") !== false ||
  isFinite("42") !== true ||
  isFinite("not-a-number") !== false ||
  encodeURI("front camera?x=1&name=камера") !==
    "front%20camera?x=1&name=%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0" ||
  encodeURIComponent("front camera?x=1&name=камера") !==
    "front%20camera%3Fx%3D1%26name%3D%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0" ||
  decodeURI("front%20camera%3Fx=1%26name=%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0") !==
    "front camera%3Fx=1%26name=камера" ||
  decodeURIComponent("front%20camera%3Fx%3D1%26name%3D%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0") !==
    "front camera?x=1&name=камера" ||
  !invalidPercent ||
  !invalidUtf8
) {
  throw new Test262Error("global utility function behavior was unexpected");
}

42
