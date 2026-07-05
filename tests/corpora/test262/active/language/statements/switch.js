let total = 0;
switch ("camera") {
    case "sensor":
        total = 1;
        break;
    case "camera":
        total = total + 20;
    case "lens":
        total = total + 22;
        break;
    default:
        total = 0;
}

if (total !== 42) {
    throw new Test262Error("switch did not match and fall through as expected");
}

let selected = "none";
switch (2) {
    case 1:
        selected = "one";
        break;
    default:
        selected = "default";
        break;
    case 2:
        selected = "two";
        break;
}

if (selected !== "two") {
    throw new Test262Error("switch default ran before a later matching case");
}

let fallback = 0;
switch ("missing") {
    case "camera":
        fallback = 1;
        break;
    default:
        fallback = 20;
    case "lens":
        fallback = fallback + 22;
        break;
}

if (fallback !== 42) {
    throw new Test262Error("switch default did not fall through");
}

switch (0) {
    case 1:
        var hoisted = 42;
}

if (hoisted !== undefined) {
    throw new Test262Error("switch case var declaration was not hoisted correctly");
}

42
