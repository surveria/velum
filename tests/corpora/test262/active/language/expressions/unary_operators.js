let camera = { name: "front-door", active: true };
let values = [40, 2];
let side = 0;

let erasedName = delete camera.name;
let erasedMissing = delete camera.missing;
let erasedIndex = delete values[0];
let erasedLength = delete values.length;
let erasedBinding = delete side;
let erasedUnknown = delete missingBinding;
let voidValue = void (side = 42);
let typeReport =
    typeof camera + " " +
    typeof camera.name + " " +
    typeof values[0] + " " +
    typeof missingBinding + " " +
    typeof function() {};

if (camera.name !== undefined) {
    throw new Test262Error("delete did not remove an object property");
}

if (values[0] !== undefined || values.length !== 2) {
    throw new Test262Error("delete did not preserve array length");
}

if (side !== 42 || voidValue !== undefined) {
    throw new Test262Error("void did not discard the expression result");
}

if (typeReport !== "object undefined undefined undefined function") {
    throw new Test262Error("typeof produced an unexpected report");
}

if (
    erasedName !== true ||
    erasedMissing !== true ||
    erasedIndex !== true ||
    erasedLength !== false ||
    erasedBinding !== false ||
    erasedUnknown !== true
) {
    throw new Test262Error("delete produced unexpected boolean results");
}

if (
    ~0 !== -1 ||
    ~1 !== -2 ||
    ~-1 !== 0 ||
    ~"5" !== -6 ||
    ~NaN !== -1 ||
    ~4294967295 !== 0
) {
    throw new Test262Error("bitwise NOT produced unexpected int32 results");
}

42
