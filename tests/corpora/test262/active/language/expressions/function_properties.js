let named = function namedCamera(left, right) {
    return left + right;
};
let anonymous = [function(one, two, three) {
    return one;
}][0];

if (named.length !== 2 || named.name !== "namedCamera" || named(40, 2) !== 42) {
    throw new Test262Error("named function metadata was unexpected");
}

if (anonymous.length !== 3 || anonymous.name !== "") {
    throw new Test262Error("anonymous function metadata was unexpected");
}

if (!("length" in named) || !("name" in named) || "missing" in named) {
    throw new Test262Error("function property membership was unexpected");
}

if (named.missing !== undefined) {
    throw new Test262Error("missing function property was unexpected");
}

let seen = "";
for (let key in named) {
    seen = seen + key + ";";
}
if (seen !== "") {
    throw new Test262Error("function built-in properties should not enumerate");
}

42
