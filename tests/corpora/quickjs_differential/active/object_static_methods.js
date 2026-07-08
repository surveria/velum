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

print(
    Object.create.length,
    Object.assign.length,
    Object.defineProperties.length,
    Object.values.length,
    Object.entries.length,
    Object.getOwnPropertyDescriptors.length,
    Object.is.length,
    Object.setPrototypeOf.length
);
print(values.length, values[0], values[1]);
print(entries.length, entries[0][0], entries[0][1], entries[1][0], entries[1][1]);
print(assigned.seed, assigned.alpha, assigned.beta, assigned.gamma, assigned[0], assigned[1]);
print(
    descriptors.alpha.value,
    descriptors.alpha.enumerable,
    descriptors.hidden.value,
    descriptors.hidden.enumerable
);
print(
    Object.getPrototypeOf(root),
    returned === object,
    Object.getPrototypeOf(object) === replacement,
    primitive
);
print(Object.is(NaN, NaN), Object.is(0, -0), Object.is(-0, -0));
