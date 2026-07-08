let proto = { inherited: 3 };
let object = Object.create(proto, {
  alpha: { value: 1, enumerable: true, writable: true, configurable: true },
  hidden: { value: 9 }
});
object.beta = 2;

let descriptors = Object.getOwnPropertyDescriptors(object);
let values = Object.values(object);
let entries = Object.entries(object);
let assigned = Object.assign({ seed: 7 }, object, { gamma: 4 }, null, undefined, "xy");

let root = Object.create(null);
let replacement = { marker: 1 };
let returned = Object.setPrototypeOf(object, replacement);
let primitive = Object.setPrototypeOf(7, null);

if (
  Object.create.length !== 2 ||
  Object.assign.length !== 2 ||
  Object.defineProperties.length !== 2 ||
  Object.values.length !== 1 ||
  Object.entries.length !== 1 ||
  Object.getOwnPropertyDescriptors.length !== 1 ||
  Object.is.length !== 2 ||
  Object.setPrototypeOf.length !== 2 ||
  values.length !== 2 ||
  values[0] !== 1 ||
  values[1] !== 2 ||
  entries.length !== 2 ||
  entries[0][0] !== "alpha" ||
  entries[0][1] !== 1 ||
  entries[1][0] !== "beta" ||
  entries[1][1] !== 2 ||
  assigned.seed !== 7 ||
  assigned.alpha !== 1 ||
  assigned.beta !== 2 ||
  assigned.gamma !== 4 ||
  assigned[0] !== "x" ||
  assigned[1] !== "y" ||
  descriptors.alpha.value !== 1 ||
  descriptors.alpha.enumerable !== true ||
  descriptors.hidden.value !== 9 ||
  descriptors.hidden.enumerable !== false ||
  Object.getPrototypeOf(root) !== null ||
  returned !== object ||
  Object.getPrototypeOf(object) !== replacement ||
  primitive !== 7 ||
  Object.is(NaN, NaN) !== true ||
  Object.is(0, -0) !== false ||
  Object.is(-0, -0) !== true
) {
  throw new Test262Error("Object static method behavior was unexpected");
}

42
