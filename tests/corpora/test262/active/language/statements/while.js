let index = 0;
let total = 0;

while (index < 6) {
    total = total + index;
    index = index + 1;
}

if (index !== 6 || total !== 15) {
    throw new Test262Error("while loop did not evaluate the condition and body correctly");
}

while (false) {
    var hoisted = 42;
}

if (hoisted !== undefined) {
    throw new Test262Error("var declaration in while body was not hoisted");
}

let ran = false;
while (!ran) {
    ran = true;
}

if (!ran) {
    throw new Test262Error("while body did not run while condition was truthy");
}

42
