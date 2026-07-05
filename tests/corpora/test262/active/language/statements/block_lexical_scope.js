let outer = 1;
let total = 0;
{
    let outer = 40;
    const delta = 2;
    total = outer + delta;
}

if (total !== 42 || outer !== 1 || typeof delta !== "undefined") {
    throw new Test262Error("block lexical bindings leaked outside the block");
}

let loopTotal = 0;
for (let index = 0; index < 4; index = index + 1) {
    let record = { value: index + 1 };
    loopTotal = loopTotal + record.value;
}

if (loopTotal !== 10 || typeof index !== "undefined" || typeof record !== "undefined") {
    throw new Test262Error("for lexical bindings leaked outside the loop");
}

let pair = 0;
for (let left = 20, right = 22; left < 21; left = left + 1) {
    pair = left + right;
}

if (pair !== 42 || typeof left !== "undefined" || typeof right !== "undefined") {
    throw new Test262Error("for declaration list bindings leaked outside the loop");
}

{
    var hoisted = 42;
}

if (hoisted !== 42) {
    throw new Test262Error("var declaration in a block was not hoisted");
}

let status = "";
try {
    let hidden = 1;
    throw "boom";
} catch (error) {
    let caught = 40;
    status = error + " " + caught;
} finally {
    let finalValue = 2;
    status = status + " " + finalValue;
}

if (
    status !== "boom 40 2" ||
    typeof hidden !== "undefined" ||
    typeof error !== "undefined" ||
    typeof caught !== "undefined" ||
    typeof finalValue !== "undefined"
) {
    throw new Test262Error("try, catch, or finally lexical bindings leaked");
}

42
