let nanBefore = NaN;
let infinityBefore = Infinity;
let deleteNaN = delete NaN;
let deleteInfinity = delete Infinity;
let deleteObject = delete Object;
let deleteMissing = delete missingGlobalName;

let shadow = 0;
{
  let NaN = 40;
  let Infinity = 2;
  shadow = NaN + Infinity;
}

if (
  typeof nanBefore !== "number" ||
  nanBefore === nanBefore ||
  infinityBefore !== Infinity ||
  !(Infinity > 1e300) ||
  !(-Infinity < -1e300) ||
  deleteNaN !== false ||
  deleteInfinity !== false ||
  deleteObject !== true ||
  deleteMissing !== true ||
  shadow !== 42
) {
  throw new Test262Error("global numeric constants behavior was unexpected");
}

42
