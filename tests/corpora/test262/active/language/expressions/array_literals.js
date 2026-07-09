let values = [40, 1, 2];
if (values.length !== 3) {
    throw new Test262Error("array literal length was not initialized");
}

if (values[0] !== 40 || values[2] !== 2) {
    throw new Test262Error("array literal elements were not readable by index");
}

if (values[9] !== undefined) {
    throw new Test262Error("missing array index did not evaluate to undefined");
}

let assigned = values[1] = values[0] + values[2];
if (assigned !== 42 || values[1] !== 42) {
    throw new Test262Error("array index assignment did not store the assigned value");
}

values[3] = assigned;
if (values.length !== 4 || values[3] !== 42) {
    throw new Test262Error("array index assignment did not extend length");
}

values["01"] = 7;
if (values.length !== 4 || values["01"] !== 7) {
    throw new Test262Error("non-canonical array index changed length");
}

let empty = [];
if (empty.length !== 0) {
    throw new Test262Error("empty array length was not zero");
}

let trailing = [40, 2,];
if (trailing.length !== 2 || trailing[1] !== 2) {
    throw new Test262Error("trailing comma array literal had unexpected elements");
}

let holes = [,, 42, ,];
if (holes.length !== 4) {
    throw new Test262Error("array elisions did not set length");
}

if (Object.hasOwn(holes, "0") || Object.hasOwn(holes, "1") || Object.hasOwn(holes, "3")) {
    throw new Test262Error("array elisions materialized own properties");
}

Array.prototype[0] = "proto";
let inherited = [, "own"];
let inheritedValue = inherited[0];
delete Array.prototype[0];
if (inheritedValue !== "proto" || inherited[1] !== "own") {
    throw new Test262Error("array elisions did not preserve inherited element lookup");
}

let spreadAfterHole = [, ...[1, 2]];
if (
    spreadAfterHole.length !== 3 ||
    Object.hasOwn(spreadAfterHole, "0") ||
    spreadAfterHole[1] !== 1 ||
    spreadAfterHole[2] !== 2
) {
    throw new Test262Error("array elision before spread had unexpected shape");
}

values.length + values[3] - 4
