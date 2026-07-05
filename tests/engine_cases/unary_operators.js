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

print(erasedName, erasedMissing, erasedIndex, erasedLength, erasedBinding, erasedUnknown);
print(typeReport);
print(values.length, side, voidValue);

camera.name === undefined &&
values[0] === undefined &&
values.length === 2 &&
side === 42 &&
voidValue === undefined &&
typeReport === "object undefined undefined undefined function" &&
erasedName === true &&
erasedMissing === true &&
erasedIndex === true &&
erasedLength === false &&
erasedBinding === false &&
erasedUnknown === true ? 42 : 0
